use crate::client::{GoldenPay, GoldenPaySession};
use crate::config::GoldenPayConfig;
use crate::error::GoldenPayError;
use crate::models::{
    CategoryFilter, CategoryNode, CategorySubcategory, ChatMessage, FetchOrderOptions, MarketOffer,
    Offer, OfferDetails, OfferEdit, OfferSaveResponse, OrderInfo, OrderPage, PriceCalculation,
    RunnerResponse, UserInfo,
};
use crate::offer::OfferEditBuilder;
use std::time::Duration;
use tokio::task::JoinSet;

/// Manages an authenticated [`GoldenPaySession`] with automatic reconnection
/// when the session expires (HTTP 401/403).
///
/// All request methods delegate to the inner session; if the request fails
/// with an authentication error, the manager automatically reconnects and
/// retries the request once.
///
/// # Example
///
/// ```ignore
/// use goldenpay::{SessionManager, GoldenPay, GoldenPayConfig};
///
/// let client = GoldenPay::new(config)?;
/// let mut manager = SessionManager::connect(client).await?;
/// let orders = manager.fetch_orders().await?;
/// ```
#[derive(Clone)]
pub struct SessionManager {
    client: GoldenPay,
    session: GoldenPaySession,
}

impl SessionManager {
    /// Creates a manager by connecting to `FunPay` with the given client.
    pub async fn connect(client: GoldenPay) -> Result<Self, GoldenPayError> {
        let session = client.connect().await?;
        Ok(Self { client, session })
    }

    /// Creates a manager from an existing client and session.
    #[must_use]
    pub fn new(client: GoldenPay, session: GoldenPaySession) -> Self {
        Self { client, session }
    }

    /// Explicitly reconnects the session by re-authenticating.
    pub async fn reconnect(&mut self) -> Result<(), GoldenPayError> {
        tracing::warn!("reconnecting session");
        self.session = self.client.connect().await?;
        Ok(())
    }

    /// Returns a reference to the underlying client.
    #[must_use]
    pub fn client(&self) -> &GoldenPay {
        &self.client
    }

    /// Returns a reference to the underlying session.
    #[must_use]
    pub fn session(&self) -> &GoldenPaySession {
        &self.session
    }

    /// Returns a mutable reference to the underlying session.
    pub fn session_mut(&mut self) -> &mut GoldenPaySession {
        &mut self.session
    }

    /// Returns authenticated user metadata.
    #[must_use]
    pub fn user(&self) -> &UserInfo {
        self.session.user()
    }

    /// Returns the polling interval between event checks.
    #[must_use]
    pub fn poll_interval(&self) -> Duration {
        self.session.poll_interval()
    }

    /// Returns the runtime configuration.
    #[must_use]
    pub fn config(&self) -> &GoldenPayConfig {
        self.session.config()
    }

    /// Sends a chat message to a dialog, with auto-reconnect on auth error.
    pub async fn send_message(
        &mut self,
        chat_id: &str,
        text: &str,
    ) -> Result<RunnerResponse, GoldenPayError> {
        let result = self.session.send_message(chat_id, text).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.send_message(chat_id, text).await
        } else {
            result
        }
    }

    /// Sends chat messages to multiple dialogs concurrently.
    ///
    /// Returns individual results; if an auth error is detected, the session
    /// is reconnected before returning.
    pub async fn send_messages(
        &mut self,
        messages: impl IntoIterator<Item = (String, String)>,
    ) -> Result<Vec<Result<RunnerResponse, GoldenPayError>>, GoldenPayError> {
        let mut set = JoinSet::new();
        let session = self.session.clone();
        for (chat_id, text) in messages {
            let session = session.clone();
            set.spawn(async move { session.send_message(&chat_id, &text).await });
        }

        let mut results = Vec::with_capacity(set.len());
        let mut needs_reconnect = false;
        while let Some(joined) = set.join_next().await {
            let result = joined.unwrap_or_else(|e| {
                Err(GoldenPayError::parse("send_messages", e.to_string()))
            });
            if matches_err_unauthorized(&result) {
                needs_reconnect = true;
            }
            results.push(result);
        }
        if needs_reconnect {
            self.reconnect().await?;
        }
        Ok(results)
    }

    /// Fetches multiple order pages concurrently.
    ///
    /// Returns individual results; if an auth error is detected, the session
    /// is reconnected before returning.
    pub async fn fetch_orders_batch(
        &mut self,
        order_ids: impl IntoIterator<Item = impl Into<String>>,
    ) -> Result<Vec<Result<OrderPage, GoldenPayError>>, GoldenPayError> {
        let mut set = JoinSet::new();
        let session = self.session.clone();
        for order_id in order_ids {
            let oid: String = order_id.into();
            let session = session.clone();
            set.spawn(async move { session.fetch_order_page(&oid).await });
        }

        let mut results = Vec::with_capacity(set.len());
        let mut needs_reconnect = false;
        while let Some(joined) = set.join_next().await {
            let result = joined.unwrap_or_else(|e| {
                Err(GoldenPayError::parse("fetch_orders_batch", e.to_string()))
            });
            if matches_err_unauthorized(&result) {
                needs_reconnect = true;
            }
            results.push(result);
        }
        if needs_reconnect {
            self.reconnect().await?;
        }
        Ok(results)
    }

    /// Fetches current order shortcuts from the trade page.
    pub async fn fetch_orders(&mut self) -> Result<Vec<OrderInfo>, GoldenPayError> {
        let result = self.session.fetch_orders().await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_orders().await
        } else {
            result
        }
    }

    /// Fetches orders filtered by the given options.
    pub async fn fetch_orders_with(
        &mut self,
        options: &FetchOrderOptions,
    ) -> Result<Vec<OrderInfo>, GoldenPayError> {
        self.fetch_orders().await.map(|orders| options.filter(orders))
    }

    /// Fetches only paid orders from the trade page.
    pub async fn fetch_paid_orders(&mut self) -> Result<Vec<OrderInfo>, GoldenPayError> {
        let result = self.session.fetch_paid_orders().await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_paid_orders().await
        } else {
            result
        }
    }

    /// Loads a single order page with parsed metadata and secrets.
    pub async fn fetch_order_page(
        &mut self,
        order_id: &str,
    ) -> Result<OrderPage, GoldenPayError> {
        let result = self.session.fetch_order_page(order_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_order_page(order_id).await
        } else {
            result
        }
    }

    /// Fetches messages from a chat through the runner endpoint.
    pub async fn fetch_chat_messages(
        &mut self,
        chat_id: &str,
    ) -> Result<Vec<ChatMessage>, GoldenPayError> {
        let result = self.session.fetch_chat_messages(chat_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_chat_messages(chat_id).await
        } else {
            result
        }
    }

    /// Fetches your offers for a given node.
    pub async fn fetch_my_offers(
        &mut self,
        node_id: i64,
    ) -> Result<Vec<Offer>, GoldenPayError> {
        let result = self.session.fetch_my_offers(node_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_my_offers(node_id).await
        } else {
            result
        }
    }

    /// Fetches public market offers for a given node.
    pub async fn fetch_market_offers(
        &mut self,
        node_id: i64,
    ) -> Result<Vec<MarketOffer>, GoldenPayError> {
        let result = self.session.fetch_market_offers(node_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_market_offers(node_id).await
        } else {
            result
        }
    }

    /// Loads editable offer details and dynamic custom fields.
    pub async fn fetch_offer_details(
        &mut self,
        node_id: i64,
        offer_id: i64,
    ) -> Result<OfferDetails, GoldenPayError> {
        let result = self.session.fetch_offer_details(node_id, offer_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_offer_details(node_id, offer_id).await
        } else {
            result
        }
    }

    /// Applies an offer edit patch on top of current remote values.
    pub async fn edit_offer(
        &mut self,
        node_id: i64,
        offer_id: i64,
        patch: OfferEdit,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        let result = self.session.edit_offer(node_id, offer_id, patch.clone()).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.edit_offer(node_id, offer_id, patch).await
        } else {
            result
        }
    }

    /// Applies an offer edit built through [`OfferEditBuilder`].
    pub async fn edit_offer_with(
        &mut self,
        node_id: i64,
        offer_id: i64,
        builder: OfferEditBuilder,
    ) -> Result<OfferSaveResponse, GoldenPayError> {
        self.edit_offer(node_id, offer_id, builder.build()).await
    }

    /// Calculates price information for a node.
    pub async fn calc_price(
        &mut self,
        node_id: i64,
        price: f64,
    ) -> Result<PriceCalculation, GoldenPayError> {
        let result = self.session.calc_price(node_id, price).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.calc_price(node_id, price).await
        } else {
            result
        }
    }

    /// Lists subcategories for a given node.
    pub async fn fetch_category_subcategories(
        &mut self,
        node_id: i64,
    ) -> Result<Vec<CategorySubcategory>, GoldenPayError> {
        let result = self.session.fetch_category_subcategories(node_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_category_subcategories(node_id).await
        } else {
            result
        }
    }

    /// Lists available category filters for a given node.
    pub async fn fetch_category_filters(
        &mut self,
        node_id: i64,
    ) -> Result<Vec<CategoryFilter>, GoldenPayError> {
        let result = self.session.fetch_category_filters(node_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_category_filters(node_id).await
        } else {
            result
        }
    }

    /// Fetches category filters and subcategories using a single page load.
    pub async fn fetch_category_metadata(
        &mut self,
        node_id: i64,
    ) -> Result<(Vec<CategorySubcategory>, Vec<CategoryFilter>), GoldenPayError> {
        let result = self.session.fetch_category_metadata(node_id).await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_category_metadata(node_id).await
        } else {
            result
        }
    }

    /// Fetches the full category tree from the marketplace root.
    pub async fn fetch_category_tree(
        &mut self,
    ) -> Result<Vec<CategoryNode>, GoldenPayError> {
        let result = self.session.fetch_category_tree().await;
        if matches_err_unauthorized(&result) {
            self.reconnect().await?;
            self.session.fetch_category_tree().await
        } else {
            result
        }
    }
}

fn matches_err_unauthorized<T>(result: &Result<T, GoldenPayError>) -> bool {
    matches!(
        result,
        Err(GoldenPayError::Unauthorized)
    )
}
