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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub stars: Option<i32>,
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceCalculation {
    pub input_price: f64,
    pub seller_price: Option<f64>,
    pub buyer_price: Option<f64>,
    pub commission: Option<f64>,
    pub numeric_fields: HashMap<String, f64>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerResponse {
    pub success: bool,
    pub error_message: Option<String>,
    pub objects: Vec<RunnerObject>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunnerObject {
    ChatNode(RunnerChatNode),
    OrdersCounters(RunnerOrdersCounters),
    Unknown(RunnerUnknownObject),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerChatNode {
    pub id: Option<String>,
    pub tag: Option<String>,
    pub messages: Vec<RunnerChatMessage>,
    pub html: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunnerChatMessage {
    pub id: i64,
    pub author_id: i64,
    pub html: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerOrdersCounters {
    pub tag: Option<String>,
    pub buyer: i64,
    pub seller: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunnerUnknownObject {
    pub object_type: Option<String>,
    pub id: Option<String>,
    pub tag: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OfferSaveResponse {
    pub success: bool,
    pub error_message: Option<String>,
    pub raw: serde_json::Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[non_exhaustive]
pub enum OrderStatus {
    Paid,
    Closed,
    Refunded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Offer {
    pub id: i64,
    pub node_id: i64,
    pub description: String,
    pub price: f64,
    pub currency: String,
    pub active: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferDetails {
    pub offer_id: i64,
    pub node_id: i64,
    pub current: OfferEdit,
    pub custom_fields: Vec<OfferField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferField {
    pub name: String,
    pub label: String,
    pub field_type: OfferFieldType,
    pub value: String,
    pub options: Vec<OfferFieldOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfferFieldOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategorySubcategory {
    pub id: i64,
    pub name: String,
    pub offer_count: u32,
    pub subcategory_type: CategorySubcategoryType,
    pub is_active: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategorySubcategoryType {
    Lots,
    Chips,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFilter {
    pub id: String,
    pub name: String,
    pub filter_type: CategoryFilterType,
    pub options: Vec<CategoryFilterOption>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryFilterOption {
    pub value: String,
    pub label: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CategoryFilterType {
    Select,
    RadioBox,
    Range,
    Checkbox,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotState {
    pub seen_orders: Vec<String>,
    pub seen_messages: HashMap<String, i64>,
}
