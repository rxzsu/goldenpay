use goldenpay::{GoldenPay, GoldenPayBot, GoldenPayConfig, GoldenPayEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let golden_key = std::env::var("FUNPAY_GOLDEN_KEY")?;

    let client = GoldenPay::new(
        GoldenPayConfig::builder()
            .golden_key(golden_key)
            .state_path("data/goldenpay-state.json")
            .build(),
    )?;

    let session = client.connect().await?;
    let mut bot = GoldenPayBot::new(session);

    bot.load_state().await?;
    bot.bootstrap().await?;

    bot.run(|event, _session| async move {
        match event {
            GoldenPayEvent::NewOrder(order) => {
                println!("new order: {} | {}", order.id, order.description);
            }
            GoldenPayEvent::NewMessage(message) => {
                println!("new message in {}: {:?}", message.chat_id, message.text);
            }
            _ => {}
        }

        Ok(())
    })
    .await?;

    Ok(())
}
