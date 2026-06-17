use crate::error::GoldenPayError;
use crate::models::{
    CategoryFilter, CategoryFilterOption, CategoryFilterType, CategorySubcategory,
    CategorySubcategoryType, ChatMessage, MarketOffer, Offer, OfferDetails, OfferEdit, OfferField,
    OfferFieldOption, OfferFieldType, OrderInfo, OrderPage, OrderStatus, PriceCalculation, Review,
    RunnerChatMessage, RunnerChatNode, RunnerObject, RunnerOrdersCounters, RunnerUnknownObject,
    UserInfo,
};
use crate::utils::extract_phpsessid;
use regex::Regex;
use scraper::{Html, Selector};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

static AMOUNT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)(\d+)\s*(pcs|pieces|шт|ед)\.?").unwrap());
static USER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/users/(\d+)/").unwrap());
static CHAT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/chat/(\d+)/").unwrap());
static SUM_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([\d.,]+)\s*(\S+)").unwrap());
static OFFER_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[?&]id=(\d+)").unwrap());
static USER_ID_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/users/(\d+)/?").unwrap());
static REVIEWS_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d+)").unwrap());
static RATING_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"rating-(\d+(?:\.\d+)?)").unwrap());
static SUBCAT_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"/(lots|chips)/(\d+)/?").unwrap());

pub fn parse_user(home_html: &str, set_cookies: &[String]) -> Result<UserInfo, GoldenPayError> {
    let document = Html::parse_document(home_html);
    let body_selector = Selector::parse("body").unwrap();
    let body = document
        .select(&body_selector)
        .next()
        .ok_or_else(|| GoldenPayError::parse("parse_user", "body not found"))?;
    let app_data = body
        .value()
        .attr("data-app-data")
        .ok_or(GoldenPayError::Unauthorized)?;
    let data: Value = serde_json::from_str(app_data)?;

    let user_id = data
        .get("userId")
        .and_then(Value::as_i64)
        .ok_or_else(|| GoldenPayError::parse("parse_user", "userId missing"))?;
    let csrf_token = data
        .get("csrf-token")
        .and_then(Value::as_str)
        .ok_or_else(|| GoldenPayError::parse("parse_user", "csrf-token missing"))?
        .to_string();
    let username = document
        .select(&Selector::parse("div.user-link-name").unwrap())
        .next()
        .map(|node| node.text().collect::<String>().trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(GoldenPayError::Unauthorized)?;

    Ok(UserInfo {
        id: user_id,
        username,
        csrf_token,
        phpsessid: extract_phpsessid(set_cookies),
    })
}

pub fn parse_orders(html: &str, seller_id: i64) -> Result<Vec<OrderInfo>, GoldenPayError> {
    let document = Html::parse_document(html);
    if document
        .select(&Selector::parse("div.user-link-name").unwrap())
        .next()
        .is_none()
    {
        return Err(GoldenPayError::Unauthorized);
    }

    let item_selector = Selector::parse("a.tc-item").unwrap();
    let order_selector = Selector::parse("div.tc-order").unwrap();
    let desc_selector = Selector::parse("div.order-desc").unwrap();
    let buyer_selector = Selector::parse("div.media-user-name span").unwrap();
    let muted_selector = Selector::parse("div.text-muted").unwrap();
    let mut orders = Vec::new();
    for item in document.select(&item_selector) {
        let classes: Vec<_> = item.value().classes().collect();
        let status = if classes.contains(&"warning") {
            OrderStatus::Refunded
        } else if classes.contains(&"info") {
            OrderStatus::Paid
        } else {
            OrderStatus::Closed
        };

        let id = item
            .select(&order_selector)
            .next()
            .map(|node| node.text().collect::<String>())
            .unwrap_or_default()
            .trim()
            .trim_start_matches('#')
            .to_string();
        if id.is_empty() {
            continue;
        }

        let description = item
            .select(&desc_selector)
            .next()
            .map(|node| node.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let buyer_node = item.select(&buyer_selector).next();
        let buyer_username = buyer_node
            .as_ref()
            .map(|node| node.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let buyer_id = buyer_node
            .and_then(|node| node.value().attr("data-href"))
            .and_then(|href| href.split("/users/").nth(1))
            .and_then(|tail| tail.trim_end_matches('/').parse::<i64>().ok())
            .unwrap_or_default();

        let subcategory_name = item
            .select(&muted_selector)
            .next()
            .map(|node| node.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        let amount = AMOUNT_REGEX
            .captures(&description)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse::<i32>().ok())
            .unwrap_or(1);

        orders.push(OrderInfo {
            id,
            buyer_username,
            buyer_id,
            chat_id: build_chat_id(seller_id, buyer_id),
            description,
            subcategory_name,
            amount,
            status,
        });
    }

    Ok(orders)
}

pub fn parse_chat_messages(chat_id: &str, response: &Value) -> Vec<ChatMessage> {
    parse_runner_objects(response)
        .into_iter()
        .filter_map(|object| match object {
            RunnerObject::ChatNode(node) => Some(node),
            _ => None,
        })
        .flat_map(|node| node.messages.into_iter())
        .map(|message| ChatMessage {
            id: message.id,
            chat_id: chat_id.to_string(),
            author_id: message.author_id,
            text: message.html.as_deref().and_then(extract_message_text),
        })
        .collect()
}

pub fn parse_runner_objects(response: &Value) -> Vec<RunnerObject> {
    response
        .get("objects")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .cloned()
        .map(parse_runner_object)
        .collect()
}

pub fn parse_price_calculation(response: Value, input_price: f64) -> PriceCalculation {
    let mut numeric_fields = HashMap::new();
    collect_numeric_fields(None, &response, &mut numeric_fields);

    let seller_price = detect_numeric_field(
        &numeric_fields,
        &["seller_price", "seller", "amount", "price", "sum"],
    );
    let buyer_price = detect_numeric_field(
        &numeric_fields,
        &["buyer_price", "buyer", "buyerAmount", "buyer_sum", "total"],
    );
    let commission = detect_numeric_field(
        &numeric_fields,
        &["commission", "fee", "site_fee", "siteCommission"],
    );

    PriceCalculation {
        input_price,
        seller_price,
        buyer_price,
        commission,
        numeric_fields,
        raw: response,
    }
}

pub fn parse_order_page(html: &str, order_id: &str) -> Result<OrderPage, GoldenPayError> {
    let document = Html::parse_document(html);
    if document
        .select(&Selector::parse("div.user-link-name").unwrap())
        .next()
        .is_none()
    {
        return Err(GoldenPayError::Unauthorized);
    }

    let param_selector = Selector::parse("div.param-item").unwrap();
    let h5_selector = Selector::parse("h5").unwrap();
    let div_selector = Selector::parse("div").unwrap();
    let buyer_selector = Selector::parse(".order-buyer a").unwrap();
    let sum_selector = Selector::parse(".order-sum").unwrap();
    let review_selector = Selector::parse(".review-item").unwrap();
    let chat_selector = Selector::parse("a[href*='/chat/']").unwrap();
    let secret_selector = Selector::parse("span.secret-placeholder").unwrap();
    let user_regex = &USER_REGEX;
    let chat_regex = &CHAT_REGEX;
    let sum_regex = &SUM_REGEX;

    let mut short_description = None;
    let mut full_description = None;
    let mut params = Vec::new();
    let mut amount = 0;
    let mut subcategory_name = None;
    let mut secrets = Vec::new();

    for param in document.select(&param_selector) {
        let Some(header) = param.select(&h5_selector).next() else {
            continue;
        };
        let label = header.text().collect::<String>().trim().to_string();
        let value = param
            .select(&div_selector)
            .next()
            .map(|node| node.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        match label.to_ascii_lowercase().as_str() {
            "short description" | "краткое описание" => {
                short_description = Some(value)
            }
            "full description" | "полное описание" => full_description = Some(value),
            "category" | "категория" => subcategory_name = Some(value),
            "amount" | "количество" => amount = value.parse::<i32>().unwrap_or(0),
            _ => {
                if !value.is_empty() {
                    params.push((label, value));
                }
            }
        }

        for secret in param.select(&secret_selector) {
            let text = secret.text().collect::<String>().trim().to_string();
            if !text.is_empty() {
                secrets.push(text);
            }
        }
    }

    let buyer_link = document.select(&buyer_selector).next();
    let buyer_username = buyer_link
        .as_ref()
        .map(|node| node.text().collect::<String>().trim().to_string())
        .unwrap_or_default();
    let buyer_id = buyer_link
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| user_regex.captures(href))
        .and_then(|caps| caps.get(1))
        .and_then(|m| m.as_str().parse::<i64>().ok())
        .unwrap_or_default();

    let sum_text = document
        .select(&sum_selector)
        .next()
        .map(|node| node.text().collect::<String>())
        .unwrap_or_default();
    let (sum, currency) = sum_regex
        .captures(&sum_text)
        .map(|caps| {
            let sum = caps
                .get(1)
                .map(|m| m.as_str().replace(',', ".").parse::<f64>().unwrap_or(0.0))
                .unwrap_or(0.0);
            let currency = caps
                .get(2)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            (sum, currency)
        })
        .unwrap_or((0.0, String::new()));

    let chat_id = document
        .select(&chat_selector)
        .next()
        .and_then(|node| node.value().attr("href"))
        .and_then(|href| chat_regex.captures(href))
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_default();

    let review = document.select(&review_selector).next().map(|node| Review {
        stars: Some(
            node.select(&Selector::parse(".rating-mini .fas.fa-star").unwrap())
                .count() as i32,
        ),
        text: node
            .select(&Selector::parse(".review-text").unwrap())
            .next()
            .map(|n| n.text().collect::<String>().trim().to_string()),
    });

    let status = if html.contains("text-warning") {
        OrderStatus::Refunded
    } else if html.contains("text-success") {
        OrderStatus::Closed
    } else {
        OrderStatus::Paid
    };

    Ok(OrderPage {
        id: order_id.to_string(),
        status,
        amount,
        sum,
        currency,
        buyer_id,
        buyer_username,
        chat_id,
        short_description,
        full_description,
        subcategory_name,
        secrets,
        params,
        review,
        raw_html: html.to_string(),
    })
}

pub fn parse_my_offers(html: &str, node_id: i64) -> Vec<Offer> {
    let document = Html::parse_document(html);
    let item_selector = Selector::parse("a.tc-item[data-offer]").unwrap();
    let desc_selector = Selector::parse("div.tc-desc-text").unwrap();
    let price_selector = Selector::parse("div.tc-price").unwrap();
    let unit_selector = Selector::parse("span.unit").unwrap();
    let mut offers = Vec::new();

    for item in document.select(&item_selector) {
        let offer_id = item
            .value()
            .attr("data-offer")
            .and_then(|s| s.parse::<i64>().ok())
            .unwrap_or_default();
        if offer_id == 0 {
            continue;
        }

        let description = item
            .select(&desc_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let price_node = item.select(&price_selector).next();
        let price = price_node
            .and_then(|el| el.value().attr("data-s"))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or_default();
        let currency = price_node
            .and_then(|el| el.select(&unit_selector).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        offers.push(Offer {
            id: offer_id,
            node_id,
            description,
            price,
            currency,
            active: !item.value().classes().any(|c| c == "warning"),
        });
    }

    offers
}

pub fn parse_market_offers(html: &str, node_id: i64) -> Vec<MarketOffer> {
    let document = Html::parse_document(html);
    let item_selector = Selector::parse("a.tc-item").unwrap();
    let desc_selector = Selector::parse("div.tc-desc-text").unwrap();
    let price_selector = Selector::parse("div.tc-price").unwrap();
    let unit_selector = Selector::parse("span.unit").unwrap();
    let seller_selector = Selector::parse("span.pseudo-a[data-href]").unwrap();
    let reviews_selector = Selector::parse("div.media-user-reviews").unwrap();
    let rating_count_selector = Selector::parse("span.rating-mini-count").unwrap();
    let rating_stars_selector = Selector::parse("div.rating-stars").unwrap();
    let offer_id_regex = &OFFER_ID_REGEX;
    let user_id_regex = &USER_ID_REGEX;
    let reviews_regex = &REVIEWS_REGEX;
    let rating_regex = &RATING_REGEX;
    let mut offers = Vec::new();

    for item in document.select(&item_selector) {
        let href = item.value().attr("href").unwrap_or_default();
        let offer_id = offer_id_regex
            .captures(href)
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or_default();
        if offer_id == 0 {
            continue;
        }

        let description = item
            .select(&desc_selector)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let price_node = item.select(&price_selector).next();
        let price = price_node
            .and_then(|el| el.value().attr("data-s"))
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or_default();
        let currency = price_node
            .and_then(|el| el.select(&unit_selector).next())
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let seller_node = item.select(&seller_selector).next();
        let seller_name = seller_node
            .as_ref()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();
        let seller_id = seller_node
            .and_then(|el| el.value().attr("data-href"))
            .and_then(|href| user_id_regex.captures(href))
            .and_then(|c| c.get(1))
            .and_then(|m| m.as_str().parse::<i64>().ok())
            .unwrap_or_default();
        let seller_online = item.value().attr("data-online") == Some("1");
        let is_promo = item.value().classes().any(|c| c == "offer-promo");
        let seller_reviews = item
            .select(&reviews_selector)
            .next()
            .map(|node| {
                node.select(&rating_count_selector)
                    .next()
                    .map(|n| n.text().collect::<String>())
                    .unwrap_or_else(|| node.text().collect::<String>())
            })
            .and_then(|text| {
                reviews_regex
                    .captures(&text)
                    .and_then(|c| c.get(1))
                    .and_then(|m| m.as_str().parse::<u32>().ok())
            })
            .unwrap_or_default();
        let seller_rating = item
            .select(&reviews_selector)
            .next()
            .and_then(|node| node.select(&rating_stars_selector).next())
            .and_then(|node| {
                node.value().classes().find_map(|class| {
                    rating_regex
                        .captures(class)
                        .and_then(|c| c.get(1))
                        .and_then(|m| m.as_str().parse::<f64>().ok())
                })
            });

        offers.push(MarketOffer {
            id: offer_id,
            node_id,
            description,
            price,
            currency,
            seller_id,
            seller_name,
            seller_online,
            seller_rating,
            seller_reviews,
            is_promo,
        });
    }

    offers
}

pub fn parse_offer_details(html: &str, offer_id: i64, node_id: i64) -> OfferDetails {
    let document = Html::parse_document(html);
    let form_group_selector = Selector::parse("div.form-group").unwrap();
    let label_selector = Selector::parse("label").unwrap();
    let input_selector = Selector::parse("input").unwrap();
    let textarea_selector = Selector::parse("textarea").unwrap();
    let select_selector = Selector::parse("select").unwrap();
    let option_selector = Selector::parse("option").unwrap();

    let current = OfferEdit {
        quantity: Some(extract_field_value(&document, "fields[quantity]")),
        quantity2: Some(extract_field_value(&document, "fields[quantity2]")),
        method: Some(extract_field_value(&document, "fields[method]")),
        offer_type: Some(extract_field_value(&document, "fields[type]")),
        server_id: Some(extract_field_value(&document, "server_id")),
        desc_ru: Some(extract_textarea_value(&document, "fields[desc][ru]")),
        desc_en: Some(extract_textarea_value(&document, "fields[desc][en]")),
        payment_msg_ru: Some(extract_textarea_value(&document, "fields[payment_msg][ru]")),
        payment_msg_en: Some(extract_textarea_value(&document, "fields[payment_msg][en]")),
        summary_ru: Some(extract_input_value(&document, "fields[summary][ru]")),
        summary_en: Some(extract_input_value(&document, "fields[summary][en]")),
        game: Some(extract_field_value(&document, "fields[game]")),
        images: Some(extract_input_value(&document, "fields[images]")),
        price: Some(extract_input_value(&document, "price")),
        deactivate_after_sale: Some(extract_checkbox_value(&document, "deactivate_after_sale")),
        active: Some(extract_checkbox_value(&document, "active")),
        location: Some(extract_input_value(&document, "location")),
        deleted: None,
    };

    let mut custom_fields = Vec::new();
    for group in document.select(&form_group_selector) {
        let label = group
            .select(&label_selector)
            .next()
            .map(|l| l.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if let Some(input) = group.select(&input_selector).next() {
            let name = input.value().attr("name").unwrap_or_default().to_string();
            if !name.starts_with("fields[")
                || name.contains("[desc]")
                || name.contains("[payment_msg]")
                || name.contains("[images]")
            {
                continue;
            }

            let input_type = input.value().attr("type").unwrap_or("text");
            let field_type = match input_type {
                "checkbox" => OfferFieldType::Checkbox,
                "hidden" => OfferFieldType::Hidden,
                _ => OfferFieldType::Text,
            };
            let value = if field_type == OfferFieldType::Checkbox {
                if input.value().attr("checked").is_some() {
                    "true".to_string()
                } else {
                    "false".to_string()
                }
            } else {
                input.value().attr("value").unwrap_or_default().to_string()
            };

            custom_fields.push(OfferField {
                name,
                label,
                field_type,
                value,
                options: vec![],
            });
        } else if let Some(textarea) = group.select(&textarea_selector).next() {
            let name = textarea
                .value()
                .attr("name")
                .unwrap_or_default()
                .to_string();
            if !name.starts_with("fields[")
                || name.contains("[desc]")
                || name.contains("[payment_msg]")
            {
                continue;
            }

            custom_fields.push(OfferField {
                name,
                label,
                field_type: OfferFieldType::Textarea,
                value: textarea.text().collect::<String>(),
                options: vec![],
            });
        } else if let Some(select) = group.select(&select_selector).next() {
            let name = select.value().attr("name").unwrap_or_default().to_string();
            if !name.starts_with("fields[") {
                continue;
            }

            let mut selected = String::new();
            let options = select
                .select(&option_selector)
                .map(|opt| {
                    let selected_here = opt.value().attr("selected").is_some();
                    let value = opt.value().attr("value").unwrap_or_default().to_string();
                    if selected_here {
                        selected = value.clone();
                    }
                    OfferFieldOption {
                        value,
                        label: opt.text().collect::<String>().trim().to_string(),
                        selected: selected_here,
                    }
                })
                .collect();

            custom_fields.push(OfferField {
                name,
                label,
                field_type: OfferFieldType::Select,
                value: selected,
                options,
            });
        }
    }

    OfferDetails {
        offer_id,
        node_id,
        current,
        custom_fields,
    }
}

pub fn parse_category_subcategories(html: &str) -> Vec<CategorySubcategory> {
    let document = Html::parse_document(html);
    let container_selector = Selector::parse("div.counter-list.counter-list-pills").unwrap();
    let item_selector = Selector::parse("a.counter-item").unwrap();
    let name_selector = Selector::parse("div.counter-param").unwrap();
    let count_selector = Selector::parse("div.counter-value").unwrap();
    let re = &SUBCAT_REGEX;

    let Some(container) = document.select(&container_selector).next() else {
        return vec![];
    };

    container
        .select(&item_selector)
        .filter_map(|item| {
            let href = item.value().attr("href").unwrap_or_default();
            let caps = re.captures(href)?;
            let id = caps.get(2)?.as_str().parse::<i64>().ok()?;
            let subcategory_type = match caps.get(1)?.as_str() {
                "lots" => CategorySubcategoryType::Lots,
                "chips" => CategorySubcategoryType::Chips,
                _ => return None,
            };

            Some(CategorySubcategory {
                id,
                name: item
                    .select(&name_selector)
                    .next()
                    .map(|el| el.text().collect::<String>().trim().to_string())
                    .unwrap_or_default(),
                offer_count: item
                    .select(&count_selector)
                    .next()
                    .and_then(|el| {
                        el.text()
                            .collect::<String>()
                            .trim()
                            .replace(' ', "")
                            .parse::<u32>()
                            .ok()
                    })
                    .unwrap_or_default(),
                subcategory_type,
                is_active: item.value().classes().any(|c| c == "active"),
            })
        })
        .collect()
}

pub fn parse_category_filters(html: &str) -> Vec<CategoryFilter> {
    let document = Html::parse_document(html);
    let filters_selector = Selector::parse("div.showcase-filters").unwrap();
    let lot_field_selector = Selector::parse("div.lot-field").unwrap();
    let select_selector = Selector::parse("select.lot-field-input").unwrap();
    let option_selector = Selector::parse("option").unwrap();
    let radio_box_selector = Selector::parse("div.lot-field-radio-box").unwrap();
    let button_selector = Selector::parse("button").unwrap();
    let range_box_selector = Selector::parse("div.lot-field-range-box").unwrap();
    let label_selector = Selector::parse("label.control-label").unwrap();
    let checkbox_selector =
        Selector::parse("input[type=\"checkbox\"].showcase-filter-input").unwrap();
    let checkbox_label_selector = Selector::parse("label.showcase-filter-label").unwrap();

    let Some(container) = document.select(&filters_selector).next() else {
        return vec![];
    };

    let mut filters = Vec::new();
    for field in container.select(&lot_field_selector) {
        let Some(field_id) = field.value().attr("data-id") else {
            continue;
        };

        if let Some(select) = field.select(&select_selector).next() {
            let name = select
                .value()
                .attr("name")
                .map(|n| n.strip_prefix("f-").unwrap_or(n).to_string())
                .unwrap_or_else(|| field_id.to_string());
            let options = select
                .select(&option_selector)
                .filter_map(|opt| {
                    let value = opt.value().attr("value")?.to_string();
                    (!value.is_empty()).then(|| CategoryFilterOption {
                        value,
                        label: opt.text().collect::<String>().trim().to_string(),
                    })
                })
                .collect::<Vec<_>>();
            if !options.is_empty() {
                filters.push(CategoryFilter {
                    id: field_id.to_string(),
                    name,
                    filter_type: CategoryFilterType::Select,
                    options,
                });
            }
        } else if let Some(radio_box) = field.select(&radio_box_selector).next() {
            let options = radio_box
                .select(&button_selector)
                .filter_map(|btn| {
                    let value = btn.value().attr("value")?.to_string();
                    (!value.is_empty()).then(|| CategoryFilterOption {
                        value,
                        label: btn.text().collect::<String>().trim().to_string(),
                    })
                })
                .collect::<Vec<_>>();
            if !options.is_empty() {
                filters.push(CategoryFilter {
                    id: field_id.to_string(),
                    name: field_id.to_string(),
                    filter_type: CategoryFilterType::RadioBox,
                    options,
                });
            }
        } else if field.select(&range_box_selector).next().is_some() {
            filters.push(CategoryFilter {
                id: field_id.to_string(),
                name: field
                    .select(&label_selector)
                    .next()
                    .map(|n| n.text().collect::<String>().trim().to_string())
                    .unwrap_or_else(|| field_id.to_string()),
                filter_type: CategoryFilterType::Range,
                options: vec![],
            });
        }
    }

    for label in container.select(&checkbox_label_selector) {
        if let Some(checkbox) = label.select(&checkbox_selector).next() {
            let id = checkbox
                .value()
                .attr("name")
                .unwrap_or("unknown")
                .to_string();
            filters.push(CategoryFilter {
                id: id.clone(),
                name: label.text().collect::<String>().trim().to_string(),
                filter_type: CategoryFilterType::Checkbox,
                options: vec![],
            });
        }
    }

    filters
}

fn extract_message_text(html: &str) -> Option<String> {
    let fragment = Html::parse_fragment(&html.replace("<br>", "\n"));
    let text_selector = Selector::parse("div.chat-msg-text").unwrap();
    fragment
        .select(&text_selector)
        .next()
        .map(|node| node.text().collect::<String>().trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_runner_object(object: Value) -> RunnerObject {
    let object_type = object
        .get("type")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let tag = object
        .get("tag")
        .and_then(Value::as_str)
        .map(ToString::to_string);

    match object_type.as_deref() {
        Some("chat_node") => {
            let data = object.get("data");
            let messages = data
                .and_then(|value| value.get("messages"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|message| {
                    Some(RunnerChatMessage {
                        id: message.get("id").and_then(Value::as_i64)?,
                        author_id: message
                            .get("author")
                            .and_then(Value::as_i64)
                            .unwrap_or_default(),
                        html: message
                            .get("html")
                            .and_then(Value::as_str)
                            .map(ToString::to_string),
                    })
                })
                .collect();
            let html = data
                .and_then(|value| value.get("html"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            RunnerObject::ChatNode(RunnerChatNode {
                id,
                tag,
                messages,
                html,
            })
        }
        Some("orders_counters") => {
            let data = object.get("data");
            RunnerObject::OrdersCounters(RunnerOrdersCounters {
                tag,
                buyer: data
                    .and_then(|value| value.get("buyer"))
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                seller: data
                    .and_then(|value| value.get("seller"))
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
            })
        }
        _ => RunnerObject::Unknown(RunnerUnknownObject {
            object_type,
            id,
            tag,
            raw: object,
        }),
    }
}

fn collect_numeric_fields(prefix: Option<&str>, value: &Value, out: &mut HashMap<String, f64>) {
    match value {
        Value::Number(number) => {
            if let (Some(key), Some(parsed)) = (prefix, number.as_f64()) {
                out.insert(key.to_string(), parsed);
            }
        }
        Value::String(text) => {
            if let (Some(key), Some(parsed)) = (prefix, parse_numeric_string(text)) {
                out.insert(key.to_string(), parsed);
            }
        }
        Value::Object(map) => {
            for (key, nested) in map {
                let next = prefix
                    .map(|prefix| format!("{prefix}.{key}"))
                    .unwrap_or_else(|| key.clone());
                collect_numeric_fields(Some(&next), nested, out);
            }
        }
        Value::Array(items) => {
            for (index, nested) in items.iter().enumerate() {
                let next = prefix.map(|prefix| format!("{prefix}[{index}]"));
                collect_numeric_fields(next.as_deref(), nested, out);
            }
        }
        _ => {}
    }
}

fn parse_numeric_string(text: &str) -> Option<f64> {
    let normalized = text.trim().replace(',', ".");
    if normalized.is_empty() {
        return None;
    }

    normalized.parse::<f64>().ok()
}

fn detect_numeric_field(fields: &HashMap<String, f64>, aliases: &[&str]) -> Option<f64> {
    fields.iter().find_map(|(key, value)| {
        aliases
            .iter()
            .any(|alias| key.eq_ignore_ascii_case(alias) || key.ends_with(&format!(".{alias}")))
            .then_some(*value)
    })
}

fn build_chat_id(seller_id: i64, buyer_id: i64) -> String {
    let left = seller_id.min(buyer_id);
    let right = seller_id.max(buyer_id);
    format!("users-{left}-{right}")
}

fn extract_input_value(doc: &Html, name: &str) -> String {
    let selector = Selector::parse(&format!("input[name=\"{name}\"]"))
        .unwrap_or_else(|_| Selector::parse("input").unwrap());
    doc.select(&selector)
        .next()
        .and_then(|el| el.value().attr("value"))
        .unwrap_or_default()
        .to_string()
}

fn extract_textarea_value(doc: &Html, name: &str) -> String {
    let selector = Selector::parse(&format!("textarea[name=\"{name}\"]"))
        .unwrap_or_else(|_| Selector::parse("textarea").unwrap());
    doc.select(&selector)
        .next()
        .map(|el| el.text().collect::<String>())
        .unwrap_or_default()
}

fn extract_checkbox_value(doc: &Html, name: &str) -> bool {
    let selector = Selector::parse(&format!("input[name=\"{name}\"][type=\"checkbox\"]"))
        .unwrap_or_else(|_| Selector::parse("input").unwrap());
    doc.select(&selector)
        .next()
        .map(|el| el.value().attr("checked").is_some())
        .unwrap_or(false)
}

fn extract_select_value(doc: &Html, name: &str) -> String {
    let selector = Selector::parse(&format!("select[name=\"{name}\"] option[selected]"))
        .unwrap_or_else(|_| Selector::parse("select").unwrap());
    doc.select(&selector)
        .next()
        .and_then(|el| el.value().attr("value"))
        .unwrap_or_default()
        .to_string()
}

fn extract_field_value(doc: &Html, name: &str) -> String {
    let input = extract_input_value(doc, name);
    if input.is_empty() {
        extract_select_value(doc, name)
    } else {
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        std::fs::read_to_string(format!(
            "{}/tests/fixtures/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    #[test]
    fn parses_orders_trade_fixture() {
        let html = fixture("orders_trade.html");
        let orders = parse_orders(&html, 111).unwrap();

        assert_eq!(orders.len(), 1);
        assert_eq!(orders[0].id, "A1B2C3D4");
        assert_eq!(orders[0].buyer_username, "BuyerOne");
        assert_eq!(orders[0].chat_id, "users-111-222");
        assert_eq!(orders[0].subcategory_name, "Steam Keys");
        assert_eq!(orders[0].amount, 2);
        assert_eq!(orders[0].status, OrderStatus::Paid);
    }

    #[test]
    fn parses_order_page_fixture() {
        let html = fixture("order_page.html");
        let order = parse_order_page(&html, "A1B2C3D4").unwrap();

        assert_eq!(order.id, "A1B2C3D4");
        assert_eq!(order.buyer_username, "BuyerOne");
        assert_eq!(order.chat_id, "123456");
        assert_eq!(order.amount, 2);
        assert_eq!(order.subcategory_name.as_deref(), Some("Steam Keys"));
        assert_eq!(order.secrets.len(), 2);
        assert_eq!(order.review.as_ref().and_then(|r| r.stars), Some(3));
    }

    #[test]
    fn parses_offer_details_fixture() {
        let html = fixture("offer_edit.html");
        let details = parse_offer_details(&html, 99, 77);

        assert_eq!(details.offer_id, 99);
        assert_eq!(details.node_id, 77);
        assert_eq!(details.current.quantity.as_deref(), Some("10"));
        assert_eq!(details.current.price.as_deref(), Some("499"));
        assert_eq!(
            details.current.desc_ru.as_deref(),
            Some("Offer description")
        );
        assert_eq!(details.current.active, Some(true));
        assert!(
            details
                .custom_fields
                .iter()
                .any(|f| f.name == "fields[server]")
        );
    }

    #[test]
    fn parses_chat_runner_fixture() {
        let raw = fixture("chat_runner.json");
        let value: Value = serde_json::from_str(&raw).unwrap();
        let messages = parse_chat_messages("users-111-222", &value);
        let objects = parse_runner_objects(&value);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].id, 10);
        assert_eq!(messages[0].author_id, 222);
        assert_eq!(messages[0].text.as_deref(), Some("Hello\nworld"));
        assert!(matches!(objects.first(), Some(RunnerObject::ChatNode(_))));
    }

    #[test]
    fn parses_price_calculation_payload() {
        let value = serde_json::json!({
            "seller": "100",
            "buyer": 104.5,
            "commission": "4.5",
            "meta": {
                "site_fee": 4.5
            }
        });

        let price = parse_price_calculation(value, 100.0);
        assert_eq!(price.input_price, 100.0);
        assert_eq!(price.seller_price, Some(100.0));
        assert_eq!(price.buyer_price, Some(104.5));
        assert_eq!(price.commission, Some(4.5));
        assert_eq!(price.numeric_fields.get("meta.site_fee"), Some(&4.5));
    }
}
