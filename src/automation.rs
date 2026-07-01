//! Delivery automation: inventory management, order matching, and message building.

use crate::client::GoldenPaySession;
use crate::error::GoldenPayError;
use crate::models::{OrderInfo, OrderStatus, RunnerResponse};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tokio::fs;
use tokio::sync::Mutex;

/// A single deliverable item (e.g., a game key or account credential).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryItem {
    pub value: String,
}

/// Controls how items are rendered in the delivery message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryItemFormat {
    /// One item per line, no numbering.
    PlainLines,
    /// Numbered list: `1. item`, `2. item`, etc.
    Numbered,
    /// Wrapped in a markdown code block.
    CodeBlock,
}

/// An inventory of [`DeliveryItem`]s for a given product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProductInventory {
    pub items: Vec<DeliveryItem>,
}

/// The result of matching an order against available inventory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryMatch {
    pub product_key: String,
    pub items: Vec<DeliveryItem>,
}

/// A completed delivery: which items were sent for which order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryResult {
    pub order_id: String,
    pub product_key: String,
    pub delivered: Vec<DeliveryItem>,
}

/// A reserved (but not yet committed) delivery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReservedDelivery {
    pub result: DeliveryResult,
}

/// The full outcome of `DeliveryService::process_paid_order`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessPaidOrderResult {
    pub delivery: DeliveryResult,
    pub message_text: String,
    pub runner_response: RunnerResponse,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryMessageBuilder {
    pub greeting: String,
    pub intro: String,
    pub item_format: DeliveryItemFormat,
    pub include_order_id: bool,
    pub include_product_key: bool,
    pub footer: Option<String>,
    pub template: Option<String>,
}

impl Default for DeliveryMessageBuilder {
    fn default() -> Self {
        Self {
            greeting: "Thanks for your purchase!".to_string(),
            intro: "Your item:".to_string(),
            item_format: DeliveryItemFormat::Numbered,
            include_order_id: true,
            include_product_key: true,
            footer: Some("If you have any questions, reply in this chat.".to_string()),
            template: None,
        }
    }
}

impl DeliveryMessageBuilder {
    /// Creates a builder with default settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the greeting line (e.g. "Thanks for your purchase!").
    pub fn greeting(mut self, value: impl Into<String>) -> Self {
        self.greeting = value.into();
        self
    }

    /// Sets the intro line before the item list.
    pub fn intro(mut self, value: impl Into<String>) -> Self {
        self.intro = value.into();
        self
    }

    /// Sets how items are formatted (`PlainLines`, `Numbered`, or `CodeBlock`).
    #[must_use]
    pub fn item_format(mut self, value: DeliveryItemFormat) -> Self {
        self.item_format = value;
        self
    }

    /// Whether to include the order ID in the message.
    #[must_use]
    pub fn include_order_id(mut self, value: bool) -> Self {
        self.include_order_id = value;
        self
    }

    /// Whether to include the product key (subcategory name) in the message.
    #[must_use]
    pub fn include_product_key(mut self, value: bool) -> Self {
        self.include_product_key = value;
        self
    }

    /// Sets a closing footer line.
    pub fn footer(mut self, value: impl Into<String>) -> Self {
        self.footer = Some(value.into());
        self
    }

    /// Uses a custom template with `{buyer}`, `{order_id}`, `{product_key}`, `{items}` placeholders.
    pub fn template(mut self, value: impl Into<String>) -> Self {
        self.template = Some(value.into());
        self
    }

    /// Removes the custom template, reverting to the default format.
    #[must_use]
    pub fn no_template(mut self) -> Self {
        self.template = None;
        self
    }

    /// Removes the footer from the message.
    #[must_use]
    pub fn no_footer(mut self) -> Self {
        self.footer = None;
        self
    }

    /// Formats delivery items according to the configured [`DeliveryItemFormat`].
    #[must_use]
    pub fn format_items(&self, items: &[DeliveryItem]) -> String {
        match self.item_format {
            DeliveryItemFormat::PlainLines => items
                .iter()
                .map(|item| item.value.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
            DeliveryItemFormat::Numbered => items
                .iter()
                .enumerate()
                .map(|(index, item)| format!("{}. {}", index + 1, item.value))
                .collect::<Vec<_>>()
                .join("\n"),
            DeliveryItemFormat::CodeBlock => format!(
                "```\n{}\n```",
                items
                    .iter()
                    .map(|item| item.value.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
        }
    }

    #[must_use]
    pub fn build_message(&self, order: &OrderInfo, result: &DeliveryResult) -> String {
        let items_block = self.format_items(&result.delivered);

        if let Some(template) = &self.template {
            return template
                .replace("{buyer}", &order.buyer_username)
                .replace("{order_id}", &result.order_id)
                .replace("{product_key}", &result.product_key)
                .replace("{items}", &items_block);
        }

        let mut lines = vec![self.greeting.clone()];

        if self.include_order_id {
            lines.push(format!("Order: #{}", result.order_id));
        }

        if self.include_product_key {
            lines.push(format!("Product: {}", result.product_key));
        }

        lines.push(format!("Buyer: {}", order.buyer_username));
        lines.push(self.intro.clone());
        lines.push(items_block);

        if let Some(footer) = &self.footer {
            lines.push(footer.clone());
        }

        lines.join("\n")
    }
}

/// Errors that can occur during delivery processing.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DeliveryError {
    /// No product matched the order's subcategory.
    #[error("product not found")]
    ProductNotFound,
    /// The inventory has fewer items than the order requires.
    #[error("not enough items available: requested {requested}, available {available}")]
    NotEnoughItems { requested: usize, available: usize },
    /// This order was already delivered (tracked by [`DeliveryStore`]).
    #[error("order was already delivered")]
    AlreadyDelivered,
    /// The order status is not [`OrderStatus::Paid`].
    #[error("order is not paid: status={status:?}")]
    OrderNotPaid { status: OrderStatus },
    /// The runner API rejected the delivery message.
    #[error("delivery message was rejected: {message}")]
    MessageSendFailed { message: String },
}

/// Determines whether a product matches an order.
pub trait ProductMatcher: Send + Sync {
    /// Returns `true` if `product_key` matches the given `order`.
    fn matches(&self, product_key: &str, order: &OrderInfo) -> bool;
}

/// Abstraction for sending delivery messages (testable via mock).
#[async_trait]
pub trait DeliveryMessenger: Send + Sync {
    /// Sends a delivery message to the given chat.
    async fn send_delivery_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<RunnerResponse, GoldenPayError>;
}

#[async_trait]
impl DeliveryMessenger for GoldenPaySession {
    async fn send_delivery_message(
        &self,
        chat_id: &str,
        text: &str,
    ) -> Result<RunnerResponse, GoldenPayError> {
        self.send_message(chat_id, text).await
    }
}

/// Matches orders whose subcategory name exactly equals the product key.
pub struct ExactSubcategoryMatcher;

impl ProductMatcher for ExactSubcategoryMatcher {
    fn matches(&self, product_key: &str, order: &OrderInfo) -> bool {
        product_key == order.subcategory_name
    }
}

#[derive(Debug, Clone, Default)]
pub struct DeliveryService {
    pub products: HashMap<String, ProductInventory>,
}

impl DeliveryService {
    /// Creates an empty delivery service with no products.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a product with its available inventory.
    pub fn add_product(
        &mut self,
        product_key: impl Into<String>,
        items: impl IntoIterator<Item = DeliveryItem>,
    ) {
        self.products.insert(
            product_key.into(),
            ProductInventory {
                items: items.into_iter().collect(),
            },
        );
    }

    /// Finds the matching product for an order and reserves items (without removing them).
    pub fn match_order<M: ProductMatcher>(
        &self,
        matcher: &M,
        order: &OrderInfo,
    ) -> Result<DeliveryMatch, DeliveryError> {
        let Some((product_key, inventory)) = self
            .products
            .iter()
            .find(|(key, _)| matcher.matches(key, order))
        else {
            return Err(DeliveryError::ProductNotFound);
        };

        let requested = order.amount.max(0) as usize;
        let available = inventory.items.len();
        if available < requested {
            return Err(DeliveryError::NotEnoughItems {
                requested,
                available,
            });
        }

        Ok(DeliveryMatch {
            product_key: product_key.clone(),
            items: inventory.items.iter().take(requested).cloned().collect(),
        })
    }

    /// Removes matched items from inventory and returns them as a delivery.
    pub fn deliver<M: ProductMatcher>(
        &mut self,
        matcher: &M,
        order: &OrderInfo,
    ) -> Result<DeliveryResult, DeliveryError> {
        let matched = self.match_order(matcher, order)?;
        let inventory = self
            .products
            .get_mut(&matched.product_key)
            .expect("inventory must exist after successful match");
        let delivered = inventory
            .items
            .drain(0..matched.items.len())
            .collect::<Vec<_>>();

        Ok(DeliveryResult {
            order_id: order.id.clone(),
            product_key: matched.product_key,
            delivered,
        })
    }

    /// Reserves items for an order (same as `deliver`, but returns a [`ReservedDelivery`]).
    pub fn reserve<M: ProductMatcher>(
        &mut self,
        matcher: &M,
        order: &OrderInfo,
    ) -> Result<ReservedDelivery, DeliveryError> {
        Ok(ReservedDelivery {
            result: self.deliver(matcher, order)?,
        })
    }

    /// Returns reserved items back to the inventory (e.g., after a failed send).
    pub fn release_reserved(&mut self, reserved: ReservedDelivery) {
        let inventory = self
            .products
            .entry(reserved.result.product_key.clone())
            .or_default();

        let mut restored = reserved.result.delivered;
        restored.append(&mut inventory.items);
        inventory.items = restored;
    }

    /// Returns the number of items still available for a product.
    #[must_use]
    pub fn remaining_items(&self, product_key: &str) -> Option<usize> {
        self.products.get(product_key).map(|inventory| inventory.items.len())
    }

    /// Delivers an order with deduplication via the given [`DeliveryStore`].
    pub async fn deliver_order<M: ProductMatcher, S: DeliveryStore>(
        &mut self,
        matcher: &M,
        store: &S,
        order: &OrderInfo,
    ) -> Result<DeliveryResult, GoldenPayError> {
        if store.contains_order(&order.id).await {
            return Err(DeliveryError::AlreadyDelivered.into());
        }

        let result = self.deliver(matcher, order)?;
        store.claim_pending(&result).await?;
        store.commit_delivered(&result).await?;
        Ok(result)
    }

    pub async fn process_paid_order<M, S, T>(
        &mut self,
        matcher: &M,
        store: &S,
        messenger: &T,
        builder: &DeliveryMessageBuilder,
        order: &OrderInfo,
    ) -> Result<ProcessPaidOrderResult, GoldenPayError>
    where
        M: ProductMatcher,
        S: DeliveryStore,
        T: DeliveryMessenger,
    {
        if order.status != OrderStatus::Paid {
            return Err(DeliveryError::OrderNotPaid {
                status: order.status,
            }
            .into());
        }

        if store.contains_order(&order.id).await {
            return Err(DeliveryError::AlreadyDelivered.into());
        }

        let reserved = self.reserve(matcher, order)?;
        store.claim_pending(&reserved.result).await?;

        let message_text = builder.build_message(order, &reserved.result);
        let runner_response = messenger
            .send_delivery_message(&order.chat_id, &message_text)
            .await?;
        if !runner_response.success {
            self.release_reserved(reserved);
            store.release_pending(&order.id).await?;
            return Err(DeliveryError::MessageSendFailed {
                message: runner_response
                    .error_message
                    .clone()
                    .unwrap_or_else(|| "runner response reported failure".to_string()),
            }
            .into());
        }
        store.commit_delivered(&reserved.result).await?;
        let delivery = reserved.result;

        Ok(ProcessPaidOrderResult {
            delivery,
            message_text,
            runner_response,
        })
    }
}

/// A persisted record of a delivery with its current status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveredOrderRecord {
    pub order_id: String,
    pub product_key: String,
    pub delivered: Vec<DeliveryItem>,
    pub status: DeliveryRecordStatus,
}

/// Whether a delivery record is pending confirmation or fully committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryRecordStatus {
    /// Items were reserved but not yet sent.
    Pending,
    /// Items were sent and confirmed.
    Delivered,
}

/// Persistence for tracking delivered orders and preventing duplicates.
#[async_trait]
pub trait DeliveryStore: Send + Sync {
    /// Returns `true` if the order ID has been recorded.
    async fn contains_order(&self, order_id: &str) -> bool;
    /// Marks an order as pending delivery (fails if already recorded).
    async fn claim_pending(&self, result: &DeliveryResult) -> Result<(), GoldenPayError>;
    /// Marks a pending order as fully delivered.
    async fn commit_delivered(&self, result: &DeliveryResult) -> Result<(), GoldenPayError>;
    /// Removes a pending order record (e.g., after a failed send).
    async fn release_pending(&self, order_id: &str) -> Result<(), GoldenPayError>;
}

/// In-memory delivery store (no persistence across restarts).
#[derive(Default)]
pub struct MemoryDeliveryStore {
    inner: Arc<Mutex<HashMap<String, DeliveredOrderRecord>>>,
}

impl MemoryDeliveryStore {
    /// Creates an empty in-memory delivery store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl DeliveryStore for MemoryDeliveryStore {
    async fn contains_order(&self, order_id: &str) -> bool {
        self.inner.lock().await.contains_key(order_id)
    }

    async fn claim_pending(&self, result: &DeliveryResult) -> Result<(), GoldenPayError> {
        let mut inner = self.inner.lock().await;
        if inner.contains_key(&result.order_id) {
            return Err(DeliveryError::AlreadyDelivered.into());
        }

        inner.insert(
            result.order_id.clone(),
            DeliveredOrderRecord {
                order_id: result.order_id.clone(),
                product_key: result.product_key.clone(),
                delivered: result.delivered.clone(),
                status: DeliveryRecordStatus::Pending,
            },
        );
        Ok(())
    }

    async fn commit_delivered(&self, result: &DeliveryResult) -> Result<(), GoldenPayError> {
        self.inner.lock().await.insert(
            result.order_id.clone(),
            DeliveredOrderRecord {
                order_id: result.order_id.clone(),
                product_key: result.product_key.clone(),
                delivered: result.delivered.clone(),
                status: DeliveryRecordStatus::Delivered,
            },
        );
        Ok(())
    }

    async fn release_pending(&self, order_id: &str) -> Result<(), GoldenPayError> {
        self.inner.lock().await.remove(order_id);
        Ok(())
    }
}

/// JSON-file-backed delivery store with atomic writes.
pub struct JsonDeliveryStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl JsonDeliveryStore {
    /// Creates a store that persists deliveries to the given file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }

    async fn load_all(&self) -> Result<HashMap<String, DeliveredOrderRecord>, GoldenPayError> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let raw = fs::read_to_string(&self.path).await?;
        Ok(serde_json::from_str(&raw)?)
    }

    async fn save_all(
        &self,
        records: &HashMap<String, DeliveredOrderRecord>,
    ) -> Result<(), GoldenPayError> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let raw = serde_json::to_string_pretty(records)?;
        let file_name = self
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                GoldenPayError::state(format!("invalid file name for {}", self.path.display()))
            })?;
        let tmp_path = self.path.with_file_name(format!("{file_name}.tmp"));
        fs::write(&tmp_path, raw).await?;
        fs::rename(&tmp_path, &self.path).await?;
        Ok(())
    }
}

#[async_trait]
impl DeliveryStore for JsonDeliveryStore {
    async fn contains_order(&self, order_id: &str) -> bool {
        let _guard = self.lock.lock().await;
        self.load_all()
            .await
            .is_ok_and(|records| records.contains_key(order_id))
    }

    async fn claim_pending(&self, result: &DeliveryResult) -> Result<(), GoldenPayError> {
        let _guard = self.lock.lock().await;
        let mut records = self.load_all().await?;
        if records.contains_key(&result.order_id) {
            return Err(DeliveryError::AlreadyDelivered.into());
        }

        records.insert(
            result.order_id.clone(),
            DeliveredOrderRecord {
                order_id: result.order_id.clone(),
                product_key: result.product_key.clone(),
                delivered: result.delivered.clone(),
                status: DeliveryRecordStatus::Pending,
            },
        );
        self.save_all(&records).await
    }

    async fn commit_delivered(&self, result: &DeliveryResult) -> Result<(), GoldenPayError> {
        let _guard = self.lock.lock().await;
        let mut records = self.load_all().await?;
        records.insert(
            result.order_id.clone(),
            DeliveredOrderRecord {
                order_id: result.order_id.clone(),
                product_key: result.product_key.clone(),
                delivered: result.delivered.clone(),
                status: DeliveryRecordStatus::Delivered,
            },
        );
        self.save_all(&records).await
    }

    async fn release_pending(&self, order_id: &str) -> Result<(), GoldenPayError> {
        let _guard = self.lock.lock().await;
        let mut records = self.load_all().await?;
        if matches!(
            records.get(order_id).map(|record| record.status),
            Some(DeliveryRecordStatus::Pending)
        ) {
            records.remove(order_id);
            self.save_all(&records).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::OrderStatus;
    use crate::models::{RunnerObject, RunnerUnknownObject};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tokio::sync::Mutex as TokioMutex;

    fn sample_order() -> OrderInfo {
        OrderInfo {
            id: "ORDER1".to_string(),
            buyer_username: "buyer".to_string(),
            buyer_id: 2,
            chat_id: "users-1-2".to_string(),
            description: "Steam".to_string(),
            subcategory_name: "Steam Keys".to_string(),
            amount: 2,
            status: OrderStatus::Paid,
        }
    }

    #[derive(Default)]
    struct TestMessenger {
        sent: Arc<TokioMutex<Vec<(String, String)>>>,
    }

    #[async_trait]
    impl DeliveryMessenger for TestMessenger {
        async fn send_delivery_message(
            &self,
            chat_id: &str,
            text: &str,
        ) -> Result<RunnerResponse, GoldenPayError> {
            self.sent
                .lock()
                .await
                .push((chat_id.to_string(), text.to_string()));
            Ok(RunnerResponse {
                success: true,
                error_message: None,
                objects: vec![RunnerObject::Unknown(RunnerUnknownObject {
                    object_type: Some("test".to_string()),
                    id: None,
                    tag: None,
                    raw: serde_json::json!({ "ok": true }),
                })],
                raw: serde_json::json!({ "ok": true }),
            })
        }
    }

    #[test]
    fn delivers_items_from_inventory() {
        let mut service = DeliveryService::new();
        service.add_product(
            "Steam Keys",
            [
                DeliveryItem {
                    value: "KEY-1".to_string(),
                },
                DeliveryItem {
                    value: "KEY-2".to_string(),
                },
                DeliveryItem {
                    value: "KEY-3".to_string(),
                },
            ],
        );

        let result = service
            .deliver(&ExactSubcategoryMatcher, &sample_order())
            .unwrap();
        assert_eq!(result.product_key, "Steam Keys");
        assert_eq!(result.delivered.len(), 2);
        assert_eq!(
            service.products["Steam Keys"].items,
            vec![DeliveryItem {
                value: "KEY-3".to_string()
            }]
        );
    }

    #[tokio::test]
    async fn delivery_store_blocks_duplicate_orders() {
        let mut service = DeliveryService::new();
        service.add_product(
            "Steam Keys",
            [
                DeliveryItem {
                    value: "KEY-1".to_string(),
                },
                DeliveryItem {
                    value: "KEY-2".to_string(),
                },
            ],
        );

        let store = MemoryDeliveryStore::new();
        let first = service
            .deliver_order(&ExactSubcategoryMatcher, &store, &sample_order())
            .await
            .unwrap();
        assert_eq!(first.delivered.len(), 2);

        let second = service
            .deliver_order(&ExactSubcategoryMatcher, &store, &sample_order())
            .await;
        assert!(matches!(
            second,
            Err(GoldenPayError::Delivery(DeliveryError::AlreadyDelivered))
        ));
    }

    #[tokio::test]
    async fn json_delivery_store_roundtrip() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("goldenpay-delivery-{stamp}.json"));
        let store = JsonDeliveryStore::new(&path);

        let result = DeliveryResult {
            order_id: "ORDERJSON".to_string(),
            product_key: "Steam Keys".to_string(),
            delivered: vec![DeliveryItem {
                value: "KEY-JSON".to_string(),
            }],
        };

        store.claim_pending(&result).await.unwrap();
        store.commit_delivered(&result).await.unwrap();
        assert!(store.contains_order("ORDERJSON").await);

        let _ = fs::remove_file(path).await;
    }

    #[test]
    fn builder_formats_numbered_delivery_message() {
        let order = sample_order();
        let result = DeliveryResult {
            order_id: order.id.clone(),
            product_key: "Steam Keys".to_string(),
            delivered: vec![
                DeliveryItem {
                    value: "KEY-1".to_string(),
                },
                DeliveryItem {
                    value: "KEY-2".to_string(),
                },
            ],
        };

        let text = DeliveryMessageBuilder::new().build_message(&order, &result);
        assert!(text.contains("Order: #ORDER1"));
        assert!(text.contains("Product: Steam Keys"));
        assert!(text.contains("1. KEY-1"));
        assert!(text.contains("2. KEY-2"));
    }

    #[tokio::test]
    async fn process_paid_order_sends_message() {
        let order = sample_order();
        let mut service = DeliveryService::new();
        let store = MemoryDeliveryStore::new();
        let messenger = TestMessenger::default();

        service.add_product(
            "Steam Keys",
            [
                DeliveryItem {
                    value: "KEY-1".to_string(),
                },
                DeliveryItem {
                    value: "KEY-2".to_string(),
                },
            ],
        );

        let processed = service
            .process_paid_order(
                &ExactSubcategoryMatcher,
                &store,
                &messenger,
                &DeliveryMessageBuilder::new(),
                &order,
            )
            .await
            .unwrap();

        assert_eq!(processed.delivery.order_id, "ORDER1");
        assert!(processed.message_text.contains("KEY-1"));
        assert_eq!(messenger.sent.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn process_paid_order_rejects_unpaid_status() {
        let mut order = sample_order();
        order.status = OrderStatus::Closed;

        let mut service = DeliveryService::new();
        let store = MemoryDeliveryStore::new();
        let messenger = TestMessenger::default();

        service.add_product(
            "Steam Keys",
            [DeliveryItem {
                value: "KEY-1".to_string(),
            }],
        );

        let error = service
            .process_paid_order(
                &ExactSubcategoryMatcher,
                &store,
                &messenger,
                &DeliveryMessageBuilder::new(),
                &order,
            )
            .await
            .unwrap_err();

        assert!(matches!(error, GoldenPayError::Delivery(_)));
    }
}
