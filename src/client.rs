//! HTTP client, authenticated session, and API request methods.

use crate::config::GoldenPayConfig;
use crate::error::GoldenPayError;
use crate::models::{
    CategoryFilter, CategoryNode, CategorySubcategory, ChatMessage, MarketOffer, Offer,
    OfferDetails, OfferEdit, OfferSaveResponse, OrderInfo, OrderPage, PriceCalculation,
    RunnerResponse, UserInfo, ProfileReview, RaiseOffersResponse, WithdrawRequest,
    StoreStatistics,
};
use crate::offer::OfferEditBuilder;
use crate::models::FetchOrderOptions;
use crate::parser::{
    parse_category_filters, parse_category_subcategories, parse_category_tree, parse_chat_messages,
    parse_market_offers, parse_my_offers, parse_offer_details, parse_order_page, parse_orders,
    parse_price_calculation, parse_runner_objects, parse_user, parse_profile_reviews, parse_balance,
};
use crate::urls::Urls;
use crate::utils::{random_tag, retry_sleep};
use reqwest::header::{ACCEPT, CONTENT_TYPE, COOKIE, ORIGIN, REFERER, SET_COOKIE, USER_AGENT};
use reqwest::{Client, Response};
use serde_json::{Value, json};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

/// A reusable HTTP client for the `FunPay` API.
///
/// Created via [`GoldenPay::new`], it holds a connection pool and configuration.
/// Call [`connect`](GoldenPay::connect) to obtain an authenticated session.
#[derive(Clone)]
pub struct GoldenPay {
    http: Client,
    config: GoldenPayConfig,
    urls: Urls,
}

/// An authenticated `FunPay` session tied to a seller account.
///
/// Provides all API operations: fetching orders, sending messages,
/// editing offers, calculating prices, and browsing the marketplace.
#[derive(Clone)]
pub struct GoldenPaySession {
    http: Client,
    config: GoldenPayConfig,
    urls: Urls,
    user: UserInfo,
    rate_limiter: Option<Arc<Semaphore>>,
}

impl GoldenPay {
    /// Creates a reusable client from configuration.
    ///
    /// Returns `MissingGoldenKey` if the golden key is empty.
    pub fn new(config: GoldenPayConfig) -> Result<Self, GoldenPayError> {
        if config.golden_key.trim().is_empty() {
            return Err(GoldenPayError::MissingGoldenKey);
        }

        let mut builder = Client::builder().cookie_store(false);
        if let Some(proxy) = &config.proxy {
            builder = builder.proxy(reqwest::Proxy::all(proxy)?);
        }

        Ok(Self {
            http: builder.build()?,
            urls: Urls::new(config.base_url.clone()),
            config,
        })
    }

    /// Returns the immutable runtime configuration.
    #[must_use]
    pub fn config(&self) -> &GoldenPayConfig {
        &self.config
    }

    /// Updates the golden key used for authentication.
    ///
    /// The new key is used on the next [`connect`](GoldenPay::connect) call.
    pub fn set_golden_key(&mut self, key: impl Into<String>) {
        self.config.golden_key = key.into();
    }

    /// Establishes an authenticated session and fetches seller metadata.
    pub async fn connect(&self) -> Result<GoldenPaySession, GoldenPayError> {
        tracing::info!("connecting to FunPay");

        let response = self
            .request_with_retry(|| {
                self.http
                    .get(self.urls.home())
                    .header(USER_AGENT, &self.config.user_agent)
                    .header(
                        COOKIE,
                        format!("golden_key={}; cookie_prefs=1", self.config.golden_key),
                    )
            })
            .await?;

        let set_cookies = collect_set_cookies(&response);
        let body = response.text().await?;
        let user = parse_user(&body, &set_cookies)?;

        tracing::info!(username = %user.username, "connected");

        Ok(GoldenPaySession {
            http: self.http.clone(),
            config: self.config.clone(),
            urls: self.urls.clone(),
            user,
            rate_limiter: self.config.max_concurrent_requests.map(|max| {
                Arc::new(Semaphore::new(max.get()))
            }),
        })
    }

    async fn request_with_retry<F>(&self, build: F) -> Result<Response, GoldenPayError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        request_with_retry(&self.config, build).await
    }

    /// Checks if the configured proxy works correctly by requesting the home page.
    ///
    /// Returns `Ok(true)` if the proxy works, `Ok(false)` if no proxy is set, or `Err` if the request fails.
    pub async fn validate_proxy(&self) -> Result<bool, GoldenPayError> {
        if self.config.proxy.is_none() {
            return Ok(false);
        }

        let res = self.http
            .get(self.urls.home())
            .header(USER_AGENT, &self.config.user_agent)
            .send()
            .await;

        match res {
            Ok(response) if response.status().is_success() => Ok(true),
            Ok(_) => Ok(false),
            Err(e) => Err(GoldenPayError::Http { source: e }),
        }
    }
}

impl GoldenPaySession {
    /// Returns authenticated user metadata.
    #[must_use]
    pub fn user(&self) -> &UserInfo {
        &self.user
    }

    /// Returns the polling interval between event checks.
    #[must_use]
    pub fn poll_interval(&self) -> std::time::Duration {
        self.config.poll_interval
    }

    /// Returns the runtime configuration.
    #[must_use]
    pub fn config(&self) -> &GoldenPayConfig {
        &self.config
    }

    /// Sends chat messages to multiple dialogs concurrently.
    ///
    /// Returns a vector of results in the same order as the input.
    pub async fn send_messages(
        &self,
        messages: impl IntoIterator<Item = (String, String)>,
    ) -> Vec<Result<RunnerResponse, GoldenPayError>> {
        let mut set = JoinSet::new();
        for (chat_id, text) in messages {
            let session = self.clone();
            set.spawn(async move { session.send_message(&chat_id, &text).await });
        }

        let mut results = Vec::with_capacity(set.len());
        while let Some(joined) = set.join_next().await {
            results.push(joined.unwrap_or_else(|e| Err(GoldenPayError::parse("send_messages", e.to_string()))));
        }
        results
    }

    /// Fetches multiple order pages concurrently.
    ///
    /// Returns a vector of results in the same order as the input.
    pub async fn fetch_orders_batch(
        &self,
        order_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Vec<Result<OrderPage, GoldenPayError>> {
        let mut set = JoinSet::new();
        for order_id in order_ids {
            let oid: String = order_id.into();
            let session = self.clone();
            set.spawn(async move { session.fetch_order_page(&oid).await });
        }

        let mut results = Vec::with_capacity(set.len());
        while let Some(joined) = set.join_next().await {
            results.push(joined.unwrap_or_else(|e| Err(GoldenPayError::parse("fetch_orders_batch", e.to_string()))));
        }
        results
    }

    /// Sends a chat message to a dialog.
    pub async fn send_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<RunnerResponse, GoldenPayError> {
        let objects_json = serde_json::to_string(&vec![json!({
            "type": "chat_node",
            "id": chat_id,
            "tag": random_tag(),
            "data": { "node": chat_id, "last_message": -1, "content": "" }
        })])?;

        let request_json = json!({
            "action": "chat_message",
            "data": { "node": chat_id, "last_message": -1, "content": text }
        })
        .to_string();

        let payload = format!(
            "objects={}&request={}&csrf_token={}",
            urlencoding::encode(&objects_json),
            urlencoding::encode(&request_json),
            urlencoding::encode(&self.user.csrf_token)
        );

        self.request_runner(payload).await
    }

    /// Fetches current order shortcuts from the trade page.
    pub async fn fetch_orders(&self) -> Result<Vec<OrderInfo>, GoldenPayError> {
        let response = self.get_html(self.urls.orders_trade()).await?;
        let body = response.text().await?;
        parse_orders(&body, self.user.id)
    }

    /// Fetches only paid orders from the trade page.
    pub async fn fetch_paid_orders(&self) -> Result<Vec<OrderInfo>, GoldenPayError> {
        Ok(self
            .fetch_orders()
            .await?
            .into_iter()
            .filter(|order| order.status == crate::models::OrderStatus::Paid)
            .collect())
    }

    /// Fetches orders filtered by the given options.
    pub async fn fetch_orders_with(
        &self,
        options: &FetchOrderOptions,
    ) -> Result<Vec<OrderInfo>, GoldenPayError> {
        self.fetch_orders().await.map(|orders| options.filter(orders))
    }

    /// Calculates statistics for orders matching the provided options.
    pub async fn calculate_statistics(
        &self,
        options: &FetchOrderOptions,
    ) -> Result<StoreStatistics, GoldenPayError> {
        let orders = self.fetch_orders_with(options).await?;
        
        let mut total_sales_volume = 0;
        let mut unique_buyers = std::collections::HashSet::new();
        let total_orders = orders.len();

        for order in &orders {
            total_sales_volume += order.amount as usize;
            unique_buyers.insert(order.buyer_id);
        }

        Ok(StoreStatistics {
            total_sales_volume,
            total_orders,
            unique_buyers: unique_buyers.len(),
        })
    }

    /// Loads a single order page with parsed metadata and secrets.
    pub async fn fetch_order_page(&self, order_id: &str) -> Result<OrderPage, GoldenPayError> {
        let response = self.get_html(self.urls.order_page(order_id)).await?;
        let body = response.text().await?;
        parse_order_page(&body, order_id)
    }

    /// Fetches messages from a chat through the runner endpoint.
    pub async fn fetch_chat_messages(
        &self,
        chat_id: &str,
    ) -> Result<Vec<ChatMessage>, GoldenPayError> {
        let objects_json = serde_json::to_string(&vec![json!({
            "type": "chat_node",
            "id": chat_id,
            "tag": random_tag(),
            "data": { "node": chat_id, "last_message": -1, "content": "" }
        })])?;

        let payload = format!(
            "objects={}&request=false&csrf_token={}",
            urlencoding::encode(&objects_json),
            urlencoding::encode(&self.user.csrf_token)
        );

        let response = self.request_runner(payload).await?;
        Ok(parse_chat_messages(chat_id, &response.raw))
    }

    /// Fetches your offers for a given node.
    pub async fn fetch_my_offers(&self, node_id: i64) -> Result<Vec<Offer>, GoldenPayError> {
        let response = self.get_html(self.urls.lots_trade(node_id)).await?;
        Ok(parse_my_offers(&response.text().await?, node_id))
    }

    /// Fetches public market offers for a given node.
    pub async fn fetch_market_offers(
        &self,
        node_id: i64,
    ) -> Result<Vec<MarketOffer>, GoldenPayError> {
        let response = self.get_html(self.urls.lots_page(node_id)).await?;
        Ok(parse_market_offers(&response.text().await?, node_id))
    }

    /// Loads editable offer details and dynamic custom fields.
    pub async fn fetch_offer_details(
        &self,
        node_id: i64,
        offer_id: i64,
    ) -> Result<OfferDetails, GoldenPayError> {
        let response = self.get_html(self.urls.offer_edit(node_id, offer_id)).await?;
        Ok(parse_offer_details(
            &response.text().await?,
            offer_id,
            node_id,
        ))
    }

    /// Applies an offer edit patch on top of current remote values.
    pub async fn edit_offer(
        &self,
        node_id: i64,
        offer_id: i64,
        patch: OfferEdit,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        let current = self.fetch_offer_details(node_id, offer_id).await?.current;
        let merged = current.merge(patch);
        let payload = build_offer_payload(&self.user.csrf_token, offer_id, node_id, &merged);

        let response = self
            .post_form(
                self.urls.offer_save(),
                payload,
                Some(self.urls.offer_edit(node_id, offer_id)),
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;

        Ok(parse_offer_save_response(response.json().await?))
    }

    /// Applies an offer edit built through [`OfferEditBuilder`].
    pub async fn edit_offer_with(
        &self,
        node_id: i64,
        offer_id: i64,
        builder: OfferEditBuilder,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        self.edit_offer(node_id, offer_id, builder.build()).await
    }

    /// Creates a new offer with the provided details.
    pub async fn create_offer(
        &self,
        node_id: i64,
        details: OfferEdit,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        let payload = build_offer_payload(&self.user.csrf_token, 0, node_id, &details);

        let response = self
            .post_form(
                self.urls.offer_save(),
                payload,
                Some(self.urls.lots_trade(node_id)),
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;

        Ok(parse_offer_save_response(response.json().await?))
    }

    /// Creates a new offer using an [`OfferEditBuilder`].
    pub async fn create_offer_with(
        &self,
        node_id: i64,
        builder: OfferEditBuilder,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        self.create_offer(node_id, builder.build()).await
    }


    /// Performs a lightweight health check against the home page.
    ///
    /// Returns `true` if the server responds with a success status.
    pub async fn check_connection(&self) -> bool {
        self.get_html(self.urls.home())
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    /// Fetches the current account balance from the top navigation bar.
    pub async fn fetch_balance(&self) -> Result<f64, GoldenPayError> {
        let response = self.get_html(self.urls.home()).await?;
        let html = response.text().await?;
        parse_balance(&html)
    }

    /// Calculates price information for a node.
    pub async fn calc_price(
        &self,
        node_id: i64,
        price: f64,
    ) -> Result<PriceCalculation, GoldenPayError> {
        let input_price = price;
        let price = if price.fract() == 0.0 {
            format!("{price:.0}")
        } else {
            let formatted = format!("{price:.2}");
            formatted.trim_end_matches('0').trim_end_matches('.').to_string()
        };
        let payload = format!("nodeId={node_id}&price={price}");
        let response = self
            .post_form(
                self.urls.lots_calc(),
                payload,
                None::<String>,
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;
        Ok(parse_price_calculation(response.json().await?, input_price))
    }

    /// Lists subcategories for a given node.
    pub async fn fetch_category_subcategories(
        &self,
        node_id: i64,
    ) -> Result<Vec<CategorySubcategory>, GoldenPayError> {
        let response = self.get_html(self.urls.lots_page(node_id)).await?;
        Ok(parse_category_subcategories(&response.text().await?))
    }

    /// Lists available category filters for a given node.
    pub async fn fetch_category_filters(
        &self,
        node_id: i64,
    ) -> Result<Vec<CategoryFilter>, GoldenPayError> {
        let response = self.get_html(self.urls.lots_page(node_id)).await?;
        Ok(parse_category_filters(&response.text().await?))
    }

    /// Fetches the full category tree from the marketplace root.
    pub async fn fetch_category_tree(&self) -> Result<Vec<CategoryNode>, GoldenPayError> {
        let response = self.get_html(self.urls.lots_home()).await?;
        Ok(parse_category_tree(&response.text().await?))
    }

    /// Fetches category filters and subcategories using a single page load.
    pub async fn fetch_category_metadata(
        &self,
        node_id: i64,
    ) -> Result<(Vec<CategorySubcategory>, Vec<CategoryFilter>), GoldenPayError> {
        let response = self.get_html(self.urls.lots_page(node_id)).await?;
        let body = response.text().await?;
        Ok((
            parse_category_subcategories(&body),
            parse_category_filters(&body),
        ))
    }

    /// Raises all offers in the specified game/category.
    pub async fn raise_offers(&self, node_id: i64) -> Result<RaiseOffersResponse, GoldenPayError> {
        let payload = format!(
            "game_id={node_id}&node_id={node_id}&csrf_token={}",
            urlencoding::encode(&self.user.csrf_token)
        );
        let response = self
            .post_form(
                self.urls.lots_raise(),
                payload,
                Some(self.urls.lots_trade(node_id)),
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;
        let res: RaiseOffersResponse = response.json().await?;
        Ok(res)
    }

    /// Sends a reply to a buyer's review for a given order.
    pub async fn reply_to_review(
        &self,
        order_id: &str,
        text: &str,
    ) -> Result<RunnerResponse, GoldenPayError> {
        let payload = format!(
            "id={}&text={}&csrf_token={}",
            urlencoding::encode(order_id),
            urlencoding::encode(text),
            urlencoding::encode(&self.user.csrf_token)
        );
        let response = self
            .post_form(
                format!("{}/orders/reviewReply", self.urls.base()),
                payload,
                None::<String>,
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;
        Ok(parse_runner_response(response.json().await?))
    }

    /// Fetches all received reviews from the specified user's profile.
    pub async fn fetch_profile_reviews(&self, user_id: i64) -> Result<Vec<ProfileReview>, GoldenPayError> {
        let response = self.get_html(format!("{}/users/{}/", self.urls.base(), user_id)).await?;
        let body = response.text().await?;
        Ok(parse_profile_reviews(&body))
    }

    /// Sends a heartbeat/ping to the runner endpoint to maintain online status.
    pub async fn ping(&self) -> Result<RunnerResponse, GoldenPayError> {
        let objects_json = serde_json::to_string(&vec![json!({
            "type": "chat_node",
            "id": "0",
            "tag": random_tag(),
            "data": { "node": "0", "last_message": -1, "content": "" }
        })])?;

        let payload = format!(
            "objects={}&request=false&csrf_token={}",
            urlencoding::encode(&objects_json),
            urlencoding::encode(&self.user.csrf_token)
        );

        self.request_runner(payload).await
    }

    pub async fn upload_chat_file(
        &self,
        chat_id: &str,
        file_bytes: &[u8],
        filename: &str,
    ) -> Result<serde_json::Value, GoldenPayError> {
        let response = self
            .request_with_retry(|| {
                let form = reqwest::multipart::Form::new()
                    .text("csrf_token", self.user.csrf_token.clone())
                    .text("node", chat_id.to_string())
                    .part(
                        "file",
                        reqwest::multipart::Part::bytes(file_bytes.to_vec())
                            .file_name(filename.to_string())
                            .mime_str("application/octet-stream")
                            .unwrap(),
                    );

                self.http
                    .post(self.urls.chat_upload())
                    .header(USER_AGENT, &self.config.user_agent)
                    .header(COOKIE, self.cookie_header())
                    .header("x-requested-with", "XMLHttpRequest")
                    .multipart(form)
            })
            .await?;

        let val: serde_json::Value = response.json().await?;
        Ok(val)
    }

    /// Initiates a balance withdrawal request.
    pub async fn withdraw(
        &self,
        request: &WithdrawRequest,
    ) -> Result<RunnerResponse, GoldenPayError> {
        let payload = format!(
            "csrf_token={}&currency={}&ext_currency={}&wallet={}&amount={}",
            urlencoding::encode(&self.user.csrf_token),
            urlencoding::encode(&request.currency),
            urlencoding::encode(&request.ext_currency),
            urlencoding::encode(&request.wallet),
            urlencoding::encode(&format!("{}", request.amount))
        );

        let response = self
            .post_form(
                format!("{}/withdraw", self.urls.base()),
                payload,
                None::<String>,
                "application/json, text/javascript, */*; q=0.01",
            )
            .await?;

        Ok(parse_runner_response(response.json().await?))
    }

    /// Automatically sets the offer price to undercut the lowest competitor's price.
    ///
    /// Finds the lowest public competitor price for the given category node ID
    /// and updates the offer price to `competitor_price - undercut_by`, bounded below by `min_price`.
    pub async fn undercut_price(
        &self,
        node_id: i64,
        offer_id: i64,
        undercut_by: f64,
        min_price: f64,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        let market_offers = self.fetch_market_offers(node_id).await?;
        let my_id = self.user.id;

        let min_competitor_price = market_offers
            .iter()
            .filter(|o| o.seller_id != my_id && !o.is_promo)
            .map(|o| o.price)
            .fold(None, |min, p| match min {
                Some(m) if p < m => Some(p),
                Some(m) => Some(m),
                None => Some(p),
            });

        let target_price = match min_competitor_price {
            Some(price) => {
                let undercut = price - undercut_by;
                if undercut < min_price {
                    min_price
                } else {
                    undercut
                }
            }
            None => min_price,
        };

        let patch = OfferEdit {
            price: Some(format!("{}", target_price)),
            ..Default::default()
        };
        self.edit_offer(node_id, offer_id, patch).await
    }

    /// Deactivates all active offers for the specified node.
    pub async fn deactivate_all_offers(&self, node_id: i64) -> Result<(), GoldenPayError> {
        let offers = self.fetch_my_offers(node_id).await?;
        let active_offers: Vec<_> = offers.into_iter().filter(|o| o.active).collect();
        
        for offer in active_offers {
            self.edit_offer_with(node_id, offer.id, OfferEditBuilder::new().active(false)).await?;
        }
        
        Ok(())
    }

    /// Deletes all offers for the specified node.
    pub async fn delete_all_offers(&self, node_id: i64) -> Result<(), GoldenPayError> {
        let offers = self.fetch_my_offers(node_id).await?;
        
        for offer in offers {
            self.edit_offer_with(node_id, offer.id, OfferEditBuilder::new().deleted(true)).await?;
        }
        
        Ok(())
    }


    async fn request_runner(&self, payload: String) -> Result<RunnerResponse, GoldenPayError> {
        let response = self
            .post_form(
                self.urls.runner(),
                payload,
                Some(format!("{}/chat/", self.urls.base())),
                "*/*",
            )
            .await?;
        Ok(parse_runner_response(response.json().await?))
    }

    async fn request_with_retry<F>(&self, build: F) -> Result<Response, GoldenPayError>
    where
        F: Fn() -> reqwest::RequestBuilder,
    {
        let _permit = match &self.rate_limiter {
            Some(sem) => Some(sem.acquire().await.map_err(|_| {
                GoldenPayError::parse("rate_limiter", "semaphore closed")
            })?),
            None => None,
        };
        request_with_retry(&self.config, build).await
    }

    fn cookie_header(&self) -> String {
        match &self.user.phpsessid {
            Some(session) => format!(
                "golden_key={}; cookie_prefs=1; PHPSESSID={session}",
                self.config.golden_key
            ),
            None => format!("golden_key={}; cookie_prefs=1", self.config.golden_key),
        }
    }

    fn html_get(&self, url: impl Into<String>) -> reqwest::RequestBuilder {
        self.http
            .get(url.into())
            .header(USER_AGENT, &self.config.user_agent)
            .header(COOKIE, self.cookie_header())
            .header(ACCEPT, "*/*")
    }

    async fn get_html(&self, url: impl Into<String>) -> Result<Response, GoldenPayError> {
        let url = url.into();
        self.request_with_retry(|| self.html_get(url.clone())).await
    }

    async fn post_form<R>(
        &self,
        url: impl Into<String>,
        payload: String,
        referer: Option<R>,
        accept: &str,
    ) -> Result<Response, GoldenPayError>
    where
        R: Into<String>,
    {
        let url = url.into();
        let referer = referer.map(Into::into);
        self.request_with_retry(|| {
            let mut request = self
                .http
                .post(url.clone())
                .header(USER_AGENT, &self.config.user_agent)
                .header(COOKIE, self.cookie_header())
                .header(
                    CONTENT_TYPE,
                    "application/x-www-form-urlencoded; charset=UTF-8",
                )
                .header(ACCEPT, accept)
                .header(ORIGIN, self.urls.base())
                .header("x-requested-with", "XMLHttpRequest")
                .body(payload.clone());

            if let Some(referer) = &referer {
                request = request.header(REFERER, referer);
            }

            request
        })
        .await
    }
}

async fn request_with_retry<F>(
    config: &GoldenPayConfig,
    build: F,
) -> Result<Response, GoldenPayError>
where
    F: Fn() -> reqwest::RequestBuilder,
{
    for attempt in 1..=config.retry.max_attempts {
        match ensure_success(build().send().await).await {
            Ok(response) => return Ok(response),
            Err(error) => {
                let retryable = matches!(
                    error,
                    GoldenPayError::Http { .. }
                        | GoldenPayError::RequestFailed {
                            status: 429 | 500 | 502 | 503 | 504,
                            ..
                        }
                );

                if !retryable || attempt == config.retry.max_attempts {
                    if attempt > 1 {
                        tracing::warn!(attempt, error = %error, "request failed, no more retries");
                    }
                    return Err(error);
                }

                tracing::warn!(attempt, error = %error, "request failed, retrying");
                retry_sleep(attempt, config.retry.base_delay).await;
            }
        }
    }

    // unreachable: the loop always returns
    unreachable!()
}

async fn ensure_success(
    response: Result<Response, reqwest::Error>,
) -> Result<Response, GoldenPayError> {
    let response = response?;
    let url = response.url().to_string();

    if response.status() == reqwest::StatusCode::FORBIDDEN {
        return Err(GoldenPayError::Unauthorized);
    }

    if response.status().is_success() {
        return Ok(response);
    }

    let status = response.status().as_u16();
    let body = response.text().await.unwrap_or_default();
    Err(GoldenPayError::RequestFailed {
        method: "HTTP",
        url,
        status,
        body,
    })
}

fn collect_set_cookies(response: &Response) -> Vec<String> {
    response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok().map(ToString::to_string))
        .collect()
}

fn build_offer_payload(csrf_token: &str, offer_id: i64, node_id: i64, edit: &OfferEdit) -> String {
    let mut parts = vec![
        format!("csrf_token={}", urlencoding::encode(csrf_token)),
        format!("offer_id={offer_id}"),
        format!("node_id={node_id}"),
        field("location", edit.location.as_deref()),
        field("fields[quantity]", edit.quantity.as_deref()),
        field("fields[quantity2]", edit.quantity2.as_deref()),
        field("fields[method]", edit.method.as_deref()),
        field("fields[type]", edit.offer_type.as_deref()),
        field("server_id", edit.server_id.as_deref()),
        field("fields[desc][ru]", edit.desc_ru.as_deref()),
        field("fields[desc][en]", edit.desc_en.as_deref()),
        field("fields[payment_msg][ru]", edit.payment_msg_ru.as_deref()),
        field("fields[payment_msg][en]", edit.payment_msg_en.as_deref()),
        field("fields[summary][ru]", edit.summary_ru.as_deref()),
        field("fields[summary][en]", edit.summary_en.as_deref()),
        field("fields[game]", edit.game.as_deref()),
        field("fields[images]", edit.images.as_deref()),
        field("price", edit.price.as_deref()),
    ];

    parts.push(if edit.deactivate_after_sale.unwrap_or(false) {
        field("deactivate_after_sale[]", Some("on"))
    } else {
        field("deactivate_after_sale", None)
    });

    parts.push(if edit.active.unwrap_or(true) {
        field("active", Some("on"))
    } else {
        field("active", None)
    });

    parts.push(if edit.deleted.unwrap_or(false) {
        "deleted=1".to_string()
    } else {
        "deleted=".to_string()
    });

    parts.join("&")
}

fn field(key: &str, value: Option<&str>) -> String {
    format!(
        "{}={}",
        urlencoding::encode(key),
        urlencoding::encode(value.unwrap_or_default())
    )
}

fn parse_runner_response(raw: Value) -> RunnerResponse {
    let error_message = parse_error_message(&raw);
    let success = error_message.is_none();
    let objects = parse_runner_objects(&raw);

    RunnerResponse {
        success,
        error_message,
        objects,
        raw,
    }
}

fn parse_offer_save_response(raw: Value) -> OfferSaveResponse {
    let error_message = parse_error_message(&raw);
    let success = error_message.is_none();

    OfferSaveResponse {
        success,
        error_message,
        raw,
    }
}

fn parse_error_message(raw: &Value) -> Option<String> {
    let error = raw.get("error")?;
    if error.is_null() {
        return None;
    }

    if let Some(message) = error.as_str() {
        let trimmed = message.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    Some(error.to_string())
}
