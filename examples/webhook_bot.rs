use goldenpay::webhook::{WebhookConfig, WebhookEvent, WebhookHandler, WebhookServer};
use goldenpay::GoldenPayError;
// use std::sync::Arc;

struct MyWebhookHandler;

#[async_trait::async_trait]
impl WebhookHandler for MyWebhookHandler {
    async fn handle(&self, event: WebhookEvent) -> Result<(), GoldenPayError> {
        match event {
            WebhookEvent::NewOrder(order, _) => {
                println!(
                    "Received new order! ID: {}, Amount: {:?}",
                    order.id, order.amount
                );
            }
            WebhookEvent::NewMessage(msg, _) => {
                println!(
                    "New message in chat {}: {:?}",
                    msg.chat_id, msg.text
                );
            }
            WebhookEvent::RawEvent(raw) => {
                println!("Received raw event: {}", raw.body);
            }
            _ => {}
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = WebhookConfig::default();
    config.bind_addr = "127.0.0.1:8080".parse()?;

    let handler = MyWebhookHandler;
    let server = WebhookServer::new(config, handler);

    println!("Starting Webhook server on :8080...");
    server.run().await?;

    Ok(())
}
