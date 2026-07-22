//! Data models for orders, offers, messages, categories, and bot state.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Authenticated user metadata extracted from the home page.
#[derive(Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
    /// CSRF token required for state-modifying requests.
    pub csrf_token: String,
    /// PHP session ID, if one was set during connect.
    pub phpsessid: Option<String>,
}

impl fmt::Debug for UserInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UserInfo")
            .field("id", &self.id)
            .field("username", &self.username)
            .field("csrf_token", &"***")
            .field("phpsessid", &"***")
            .finish()
    }
}

/// Statistics calculated from a list of orders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StoreStatistics {
    pub total_sales_volume: usize,
    pub total_orders: usize,
    pub unique_buyers: usize,
}

/// A single chat message.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub chat_id: String,
    pub author_id: i64,
    pub text: Option<String>,
}

/// A compact order entry from the trade page list.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderInfo {
    pub id: String,
    pub buyer_username: String,
    pub buyer_id: i64,
    pub chat_id: String,
    pub description: String,
    pub subcategory_name: String,
    pub amount: i32,
    pub status: OrderStatus,
}

/// A detailed order page with metadata and delivery secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderPage {
    pub id: String,
    pub status: OrderStatus,
    pub amount: i32,
    pub sum: f64,
    pub currency: String,
    pub buyer_id: i64,
    pub buyer_username: String,
    pub chat_id: String,
    pub short_description: Option<String>,
    pub full_description: Option<String>,
    pub subcategory_name: Option<String>,
    pub secrets: Vec<String>,
    pub params: Vec<(String, String)>,
    pub review: Option<Review>,
    pub raw_html: String,
}

/// A buyer's review left on a completed order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub stars: Option<i32>,
    pub text: Option<String>,
}

/// A buyer's review parsed from user profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReview {
    pub buyer_username: String,
    pub buyer_id: i64,
    pub stars: i32,
    pub text: Option<String>,
    pub order_id: Option<String>,
}

/// Response returned from the lots-raise endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RaiseOffersResponse {
    pub success: bool,
    pub message: Option<String>,
}

/// Request parameters for initiating a withdrawal/payout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawRequest {
    pub currency: String,
    pub ext_currency: String,
    pub wallet: String,
    pub amount: f64,
}

/// Price breakdown including seller payout, buyer cost, and commission.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCalculation {
    pub input_price: f64,
    pub seller_price: Option<f64>,
    pub buyer_price: Option<f64>,
    pub commission: Option<f64>,
    pub numeric_fields: HashMap<String, f64>,
    pub raw: serde_json::Value,
}

/// Raw response from the FunPay runner endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerResponse {
    pub success: bool,
    pub error_message: Option<String>,
    pub objects: Vec<RunnerObject>,
    pub raw: serde_json::Value,
}

/// A parsed object from the runner response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunnerObject {
    ChatNode(RunnerChatNode),
    OrdersCounters(RunnerOrdersCounters),
    Unknown(RunnerUnknownObject),
}

/// A chat node from the runner, containing messages and HTML.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerChatNode {
    pub id: Option<String>,
    pub tag: Option<String>,
    pub messages: Vec<RunnerChatMessage>,
    pub html: Option<String>,
}

/// A single message within a runner chat node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerChatMessage {
    pub id: i64,
    pub author_id: i64,
    pub html: Option<String>,
}

/// Order counters from the runner (unread counts).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerOrdersCounters {
    pub tag: Option<String>,
    pub buyer: i64,
    pub seller: i64,
}

/// An unrecognized runner object, preserved as raw JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerUnknownObject {
    pub object_type: Option<String>,
    pub id: Option<String>,
    pub tag: Option<String>,
    pub raw: serde_json::Value,
}

/// Response from the offer-save endpoint.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfferSaveResponse {
    pub success: bool,
    pub error_message: Option<String>,
    pub raw: serde_json::Value,
}

/// The status of an order on FunPay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderStatus {
    Paid,
    Closed,
    Refunded,
}

/// A seller's own offer on the trade page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub id: i64,
    pub node_id: i64,
    pub description: String,
    pub price: f64,
    pub currency: String,
    pub active: bool,
}

/// A public market offer visible to buyers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketOffer {
    pub id: i64,
    pub node_id: i64,
    pub description: String,
    pub price: f64,
    pub currency: String,
    pub seller_id: i64,
    pub seller_name: String,
    pub seller_online: bool,
    pub seller_rating: Option<f64>,
    pub seller_reviews: u32,
    pub is_promo: bool,
}

/// A partial or full edit to apply to an offer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfferEdit {
    pub quantity: Option<String>,
    pub quantity2: Option<String>,
    pub method: Option<String>,
    pub offer_type: Option<String>,
    pub server_id: Option<String>,
    pub desc_ru: Option<String>,
    pub desc_en: Option<String>,
    pub payment_msg_ru: Option<String>,
    pub payment_msg_en: Option<String>,
    pub summary_ru: Option<String>,
    pub summary_en: Option<String>,
    pub game: Option<String>,
    pub images: Option<String>,
    pub price: Option<String>,
    pub deactivate_after_sale: Option<bool>,
    pub active: Option<bool>,
    pub location: Option<String>,
    pub deleted: Option<bool>,
}

impl OfferEdit {
    /// Merges another edit on top of this one (other fields take priority).
    #[must_use]
    pub fn merge(self, other: OfferEdit) -> Self {
        Self {
            quantity: other.quantity.or(self.quantity),
            quantity2: other.quantity2.or(self.quantity2),
            method: other.method.or(self.method),
            offer_type: other.offer_type.or(self.offer_type),
            server_id: other.server_id.or(self.server_id),
            desc_ru: other.desc_ru.or(self.desc_ru),
            desc_en: other.desc_en.or(self.desc_en),
            payment_msg_ru: other.payment_msg_ru.or(self.payment_msg_ru),
            payment_msg_en: other.payment_msg_en.or(self.payment_msg_en),
            summary_ru: other.summary_ru.or(self.summary_ru),
            summary_en: other.summary_en.or(self.summary_en),
            game: other.game.or(self.game),
            images: other.images.or(self.images),
            price: other.price.or(self.price),
            deactivate_after_sale: other.deactivate_after_sale.or(self.deactivate_after_sale),
            active: other.active.or(self.active),
            location: other.location.or(self.location),
            deleted: other.deleted.or(self.deleted),
        }
    }
}

/// Full offer details including current values and custom dynamic fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferDetails {
    pub offer_id: i64,
    pub node_id: i64,
    pub current: OfferEdit,
    pub custom_fields: Vec<OfferField>,
}

/// A dynamic custom field on an offer edit form.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferField {
    pub name: String,
    pub label: String,
    pub field_type: OfferFieldType,
    pub value: String,
    pub options: Vec<OfferFieldOption>,
}

/// An option within a select-type offer field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferFieldOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

/// The input type of a custom offer field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OfferFieldType {
    Text,
    Textarea,
    Select,
    Checkbox,
    Hidden,
    Unknown(String),
}

/// A subcategory within a marketplace category.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySubcategory {
    pub id: i64,
    pub name: String,
    pub offer_count: u32,
    pub subcategory_type: CategorySubcategoryType,
    pub is_active: bool,
}

/// Whether a subcategory lists standard offers (Lots) or virtual chips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategorySubcategoryType {
    Lots,
    Chips,
}

/// A filter available on a category page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFilter {
    pub id: String,
    pub name: String,
    pub filter_type: CategoryFilterType,
    pub options: Vec<CategoryFilterOption>,
}

/// A single option within a category filter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFilterOption {
    pub value: String,
    pub label: String,
}

/// The UI type of a category filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategoryFilterType {
    Select,
    RadioBox,
    Range,
    Checkbox,
}

/// A node in the `FunPay` marketplace category tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryNode {
    pub id: i64,
    pub name: String,
    pub subcategory_type: Option<CategorySubcategoryType>,
    pub children: Vec<CategoryNode>,
}

/// Filter options for [`fetch_orders_with`](crate::SessionManager::fetch_orders_with).
#[derive(Debug, Clone, Default)]
pub struct FetchOrderOptions {
    /// Only return orders with this status (e.g. `Paid`, `Closed`).
    pub status: Option<OrderStatus>,
    /// Minimum order amount (inclusive).
    pub min_amount: Option<i32>,
    /// Maximum order amount (inclusive).
    pub max_amount: Option<i32>,
    /// Only return orders in this subcategory.
    pub subcategory: Option<String>,
    /// Only return orders whose buyer username contains this string (case-insensitive).
    pub buyer: Option<String>,
    /// Only return orders whose description contains this string (case-insensitive).
    pub description: Option<String>,
}

impl FetchOrderOptions {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn status(mut self, status: OrderStatus) -> Self {
        self.status = Some(status);
        self
    }

    #[must_use]
    pub fn min_amount(mut self, amount: i32) -> Self {
        self.min_amount = Some(amount);
        self
    }

    #[must_use]
    pub fn max_amount(mut self, amount: i32) -> Self {
        self.max_amount = Some(amount);
        self
    }

    #[must_use]
    pub fn subcategory(mut self, name: impl Into<String>) -> Self {
        self.subcategory = Some(name.into());
        self
    }

    /// Only return orders whose buyer username contains this string.
    pub fn buyer(mut self, name: impl Into<String>) -> Self {
        self.buyer = Some(name.into());
        self
    }

    /// Only return orders whose description contains this string.
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.description = Some(text.into());
        self
    }

    /// Returns `true` if the given order matches all set filters.
    #[must_use]
    pub fn matches(&self, order: &OrderInfo) -> bool {
        if let Some(status) = &self.status
            && &order.status != status
        {
            return false;
        }
        if let Some(min) = self.min_amount
            && order.amount < min
        {
            return false;
        }
        if let Some(max) = self.max_amount
            && order.amount > max
        {
            return false;
        }
        if let Some(sub) = &self.subcategory
            && &order.subcategory_name != sub
        {
            return false;
        }
        if let Some(buyer) = &self.buyer
            && !order
                .buyer_username
                .to_ascii_lowercase()
                .contains(&buyer.to_ascii_lowercase())
        {
            return false;
        }
        if let Some(desc) = &self.description
            && !order
                .description
                .to_ascii_lowercase()
                .contains(&desc.to_ascii_lowercase())
        {
            return false;
        }
        true
    }

    /// Filters a vector of orders, returning only matching entries.
    #[must_use]
    pub fn filter(&self, orders: Vec<OrderInfo>) -> Vec<OrderInfo> {
        orders.into_iter().filter(|o| self.matches(o)).collect()
    }
}

/// Persisted state for the polling bot (seen order IDs and message counters).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotState {
    pub seen_orders: Vec<String>,
    pub seen_messages: HashMap<String, i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_order(buyer: &str, desc: &str, amount: i32) -> OrderInfo {
        OrderInfo {
            id: "ORDER1".to_string(),
            buyer_username: buyer.to_string(),
            buyer_id: 2,
            chat_id: "users-1-2".to_string(),
            description: desc.to_string(),
            subcategory_name: "Steam Keys".to_string(),
            amount,
            status: OrderStatus::Paid,
        }
    }

    #[test]
    fn matches_combines_all_filters() {
        let order = sample_order("Alice", "Steam key", 2);
        let options = FetchOrderOptions::new()
            .status(OrderStatus::Paid)
            .min_amount(1)
            .max_amount(5)
            .subcategory("Steam Keys")
            .buyer("ali")
            .description("steam");

        assert!(options.matches(&order));
    }

    #[test]
    fn rejects_when_status_differs() {
        let mut order = sample_order("Alice", "Steam key", 1);
        order.status = OrderStatus::Closed;
        let options = FetchOrderOptions::new().status(OrderStatus::Paid);
        assert!(!options.matches(&order));
    }

    #[test]
    fn rejects_when_amount_below_min() {
        let order = sample_order("Alice", "x", 1);
        let options = FetchOrderOptions::new().min_amount(2);
        assert!(!options.matches(&order));
    }

    #[test]
    fn rejects_when_amount_above_max() {
        let order = sample_order("Alice", "x", 10);
        let options = FetchOrderOptions::new().max_amount(5);
        assert!(!options.matches(&order));
    }

    #[test]
    fn buyer_filter_is_case_insensitive() {
        let order = sample_order("AliceInWonderland", "x", 1);
        assert!(FetchOrderOptions::new().buyer("alice").matches(&order));
        assert!(FetchOrderOptions::new().buyer("WONDER").matches(&order));
        assert!(!FetchOrderOptions::new().buyer("bob").matches(&order));
    }

    #[test]
    fn description_filter_is_case_insensitive() {
        let order = sample_order("Alice", "Steam Account EU", 1);
        assert!(
            FetchOrderOptions::new()
                .description("steam")
                .matches(&order)
        );
        assert!(
            FetchOrderOptions::new()
                .description("account")
                .matches(&order)
        );
        assert!(
            !FetchOrderOptions::new()
                .description("valorant")
                .matches(&order)
        );
    }

    #[test]
    fn empty_filters_match_everything() {
        let order = sample_order("Anyone", "anything", 999);
        let options = FetchOrderOptions::default();
        assert!(options.matches(&order));
    }

    #[test]
    fn filter_returns_only_matching_orders() {
        let orders = vec![
            sample_order("Alice", "Steam key", 1),
            sample_order("Bob", "Valorant points", 5),
        ];
        let options = FetchOrderOptions::new().buyer("bob");
        let filtered = options.filter(orders);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].buyer_username, "Bob");
    }
}
