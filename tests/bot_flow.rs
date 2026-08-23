use async_trait::async_trait;
use goldenpay::{GoldenPayError};
use goldenpay::{
    BotOptions, ChatMessage, DeliveryItem, DeliveryItemFormat, DeliveryMessageBuilder,
    DeliveryMessenger, DeliveryService, EventStream, ExactSubcategoryMatcher, MemoryDeliveryStore,
    MessageFilter, OrderInfo, OrderStatus, RunnerObject, RunnerResponse, RunnerUnknownObject,
};
use std::sync::Arc;
use tokio::sync::Mutex;

fn sample_order() -> OrderInfo {
    OrderInfo {
        id: "ORDER100".to_string(),
        buyer_username: "buyer".to_string(),
        buyer_id: 200,
        chat_id: "users-100-200".to_string(),
        description: "Steam keys".to_string(),
        subcategory_name: "Steam Keys".to_string(),
        amount: 2,
        status: OrderStatus::Paid,
    }
}

#[derive(Default)]
struct TestMessenger {
    sent: Arc<Mutex<Vec<(String, String)>>>,
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
fn event_stream_and_delivery_work_together() {
    let mut stream = EventStream::default();
    let order = sample_order();
    let options = BotOptions::default();
    let filter = MessageFilter {
        ignore_author_id: options.ignore_own_messages.then_some(100),
    };

    assert!(stream.should_emit_order(&order));
    assert!(!stream.should_emit_order(&order));

    let own_message = ChatMessage {
        id: 1,
        chat_id: order.chat_id.clone(),
        author_id: 100,
        text: Some("internal".to_string()),
    };
    let buyer_message = ChatMessage {
        id: 2,
        chat_id: order.chat_id.clone(),
        author_id: 200,
        text: Some("hello".to_string()),
    };

    assert!(!stream.should_emit_message(&own_message, &filter));
    assert!(stream.should_emit_message(&buyer_message, &filter));

    let mut delivery = DeliveryService::new();
    delivery.add_product(
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

    let result = delivery.deliver(&ExactSubcategoryMatcher, &order).unwrap();
    assert_eq!(result.order_id, "ORDER100");
    assert_eq!(result.delivered.len(), 2);
}

#[tokio::test]
async fn deliver_order_uses_store_for_dedup() {
    let order = sample_order();
    let mut delivery = DeliveryService::new();
    let store = MemoryDeliveryStore::new();

    delivery.add_product(
        "Steam Keys",
        [
            DeliveryItem {
                value: "KEY-A".to_string(),
            },
            DeliveryItem {
                value: "KEY-B".to_string(),
            },
        ],
    );

    let first = delivery
        .deliver_order(&ExactSubcategoryMatcher, &store, &order)
        .await
        .unwrap();
    assert_eq!(first.delivered.len(), 2);

    let second = delivery
        .deliver_order(&ExactSubcategoryMatcher, &store, &order)
        .await;
    assert!(second.is_err());
}

#[tokio::test]
async fn process_paid_order_builds_and_sends_message() {
    let order = sample_order();
    let mut delivery = DeliveryService::new();
    let store = MemoryDeliveryStore::new();
    let messenger = TestMessenger::default();
    let builder = DeliveryMessageBuilder::new()
        .item_format(DeliveryItemFormat::CodeBlock)
        .footer("Thanks for your order");

    delivery.add_product(
        "Steam Keys",
        [
            DeliveryItem {
                value: "KEY-X".to_string(),
            },
            DeliveryItem {
                value: "KEY-Y".to_string(),
            },
        ],
    );

    let result = delivery
        .process_paid_order(
            &ExactSubcategoryMatcher,
            &store,
            &messenger,
            &builder,
            &order,
        )
        .await
        .unwrap();

    assert!(result.message_text.contains("```"));
    assert!(result.message_text.contains("KEY-X"));
    assert_eq!(messenger.sent.lock().await.len(), 1);
}