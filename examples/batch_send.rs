use goldenpay::{FetchOrderOptions, GoldenPay, GoldenPayConfig, OrderStatus};

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

    let paid = session
        .fetch_orders_with(
            &FetchOrderOptions::new()
                .status(OrderStatus::Paid)
                .min_amount(1)
                .description("Steam"),
        )
        .await?;

    if paid.is_empty() {
        println!("no paid Steam orders to follow up on");
        return Ok(());
    }

    let messages = paid
        .iter()
        .map(|order| {
            (
                order.chat_id.clone(),
                format!(
                    "Hi {}, your order #{} has been queued for delivery.",
                    order.buyer_username, order.id
                ),
            )
        })
        .collect::<Vec<_>>();

    println!("dispatching {} message(s) concurrently", messages.len());
    let results = session.send_messages(messages).await;

    for (order, result) in paid.iter().zip(results) {
        match result {
            Ok(response) if response.success => {
                println!("ok    #{} -> {}", order.id, order.chat_id);
            }
            Ok(response) => {
                eprintln!(
                    "fail  #{} -> {}: {}",
                    order.id,
                    order.chat_id,
                    response.error_message.unwrap_or_default()
                );
            }
            Err(err) => eprintln!("error #{} -> {}: {err}", order.id, order.chat_id),
        }
    }

    let order_ids = paid.iter().map(|o| o.id.as_str());
    let pages = session.fetch_orders_batch(order_ids).await;
    for (order, page) in paid.iter().zip(pages) {
        match page {
            Ok(page) => println!("page  #{} ({} secret(s))", page.id, page.secrets.len()),
            Err(err) => eprintln!("page  #{}: {err}", order.id),
        }
    }

    Ok(())
}
