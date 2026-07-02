use goldenpay::{
    GoldenPay, GoldenPayBot, GoldenPayConfig, GoldenPayEvent, WithdrawRequest,
};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let golden_key = std::env::var("FUNPAY_GOLDEN_KEY")
        .unwrap_or_else(|_| "your_golden_key_here".to_string());

    let client = GoldenPay::new(
        GoldenPayConfig::builder()
            .golden_key(golden_key)
            .state_path("data/bot-features-state.json")
            .build(),
    )?;

    // Create a bot with custom options: auto-welcome messages, auto-raising, etc.
    let mut bot = GoldenPayBot::connect(client.clone())
        .await?
        .with_welcome_message("Hello! Thank you for your order. I will process it shortly.")
        .with_auto_raise(vec![12345, 67890], Some(Duration::from_secs(7200))); // Raise node 12345 and 67890 every 2 hours

    bot.load_state().await?;
    bot.bootstrap().await?;

    // Perform a heartbeat ping to maintain online status
    let session_for_ping = bot.session().clone();
    tokio::spawn(async move {
        loop {
            match session_for_ping.ping().await {
                Ok(_) => println!("Sent online heartbeat ping"),
                Err(e) => eprintln!("Failed to send heartbeat ping: {:?}", e),
            }
            tokio::time::sleep(Duration::from_secs(300)).await; // Ping every 5 minutes
        }
    });

    let bot_session = bot.session().clone();

    // Spawn the bot execution in the background
    tokio::spawn(async move {
        if let Err(e) = bot
            .run(move |event, _session| {
                let session_clone = bot_session.clone();
                async move {
                    match event {
                        GoldenPayEvent::NewOrder(order) => {
                            println!("New order received: {}", order.id);

                            // Upload an image attachment to the chat if needed
                            let test_image_bytes = vec![0u8; 100]; // Mock image bytes
                            match session_clone
                                .upload_chat_file(&order.chat_id, &test_image_bytes, "info.png")
                                .await
                            {
                                Ok(res) => println!("Uploaded attachment successfully: {:?}", res),
                                Err(e) => eprintln!("Failed to upload attachment: {:?}", e),
                            }
                        }
                        GoldenPayEvent::NewMessage(message) => {
                            println!(
                                "New message from buyer in chat {}: {:?}",
                                message.chat_id, message.text
                            );
                        }
                        _ => {}
                    }
                    Ok(())
                }
            })
            .await
        {
            eprintln!("Bot run loop error: {:?}", e);
        }
    });

    // Let's connect directly to perform manual administrative actions (like payouts or review management)
    let session = client.connect().await?;

    // 1. Fetch profile reviews
    let user_id = session.user().id;
    println!("Fetching reviews for seller ID: {}", user_id);
    match session.fetch_profile_reviews(user_id).await {
        Ok(reviews) => {
            for r in reviews {
                println!(
                    "Review from {}: {} stars - {:?}",
                    r.buyer_username, r.stars, r.text
                );

                // Auto-reply to 5-star reviews
                if r.stars == 5 && r.text.is_some() {
                    if let Some(order_id) = r.order_id {
                        match session
                            .reply_to_review(&order_id, "Thank you so much for the feedback!")
                            .await
                        {
                            Ok(_) => println!("Replied to order review {}", order_id),
                            Err(e) => eprintln!("Failed to reply to review: {:?}", e),
                        }
                    }
                }
            }
        }
        Err(e) => eprintln!("Failed to fetch reviews: {:?}", e),
    }

    // 2. Perform balance withdrawal (Example payout)
    let _withdraw_req = WithdrawRequest {
        currency: "rub".to_string(),
        ext_currency: "yookassa_card".to_string(), // YooKassa payout to Card
        wallet: "4276000000000000".to_string(),    // Target card number
        amount: 150.0,
    };

    println!("Initiating withdrawal request for 150 RUB...");
    // Commented out to prevent accidental actual payout triggers
    /*
    match session.withdraw(&_withdraw_req).await {
        Ok(res) => println!("Withdrawal response: {:?}", res),
        Err(e) => eprintln!("Withdrawal failed: {:?}", e),
    }
    */

    // Keep the main thread alive for the background tasks
    tokio::time::sleep(Duration::from_secs(10)).await;

    Ok(())
}
