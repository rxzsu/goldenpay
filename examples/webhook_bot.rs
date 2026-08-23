use goldenpay::webhook::{WebhookConfig, WebhookEvent, WebhookHandler, WebhookServer};
use goldenpay::GoldenPayError;

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
            other => {
                println!("Unhandled event: {other:?}");
            }
        }
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = WebhookConfig {
        bind_addr: "127.0.0.1:8080".parse()?,
        ..WebhookConfig::default()
    };

    let handler = MyWebhookHandler;
    let server = WebhookServer::new(config, handler);

    println!("Starting Webhook server on :8080...");
    server.run().await?;

    Ok(())
}
