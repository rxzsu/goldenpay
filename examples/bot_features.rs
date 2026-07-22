use goldenpay::{
    GoldenPay, GoldenPayBot, GoldenPayConfig, GoldenPayEvent, SqliteStateStore, WithdrawRequest,
};
use std::sync::Arc;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let golden_key = std::env::var("FUNPAY_GOLDEN_KEY")
        .unwrap_or_else(|_| "your_golden_key_here".to_string());

    // 1. Configure the client with a proxy (if available)
    let client = GoldenPay::new(
        GoldenPayConfig::builder()
            .golden_key(golden_key)
            .proxy("http://127.0.0.1:8080") // Example proxy configuration
            .build(),
    )?;

    // 2. Validate proxy health before starting
    println!("Checking proxy status...");
    match client.validate_proxy().await {
        Ok(true) => println!("Proxy is online and successfully routing requests to FunPay!"),
        Ok(false) => println!("No proxy configured or proxy validation returned false."),
        Err(e) => eprintln!("Proxy check failed: {:?}", e),
    }

    // 3. Initialize the SQLite state store for robust bot state management
    let sqlite_store = Arc::new(SqliteStateStore::new("data/bot-state.db")?);
    let manager = goldenpay::session::SessionManager::connect(client.clone()).await?;

    // 4. Create the bot with custom options: auto-welcome, auto-raise, and a sleep schedule!
    let mut bot = GoldenPayBot::with_store(manager, sqlite_store)
        .with_welcome_message("Hello! Thank you for your order. I will process it shortly.")
        .with_auto_raise(vec![12345], Some(Duration::from_secs(7200)))
        .with_sleep_schedule(
            23,             // Sleep starts at 23:00 (11 PM)
            7,              // Sleep ends at 07:00 (7 AM)
            vec![(12345, 67890)], // Deactivate node 12345, offer 67890 during sleep hours
        );

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

    // Connect directly to perform manual administrative actions (like competitor pricing & payouts)
    let session = client.connect().await?;

    // 5. Competitor pricing (Auto-undercut)
    println!("Checking competitor prices to undercut them...");
    match session
        .undercut_price(
            12345,  // Node ID
            67890,  // Offer ID
            1.5,    // Undercut by 1.5 RUB
            250.0,  // Bounded below by a minimum price of 250 RUB
        )
        .await
    {
        Ok(res) => println!("Offer price updated successfully: {:?}", res.success),
        Err(e) => eprintln!("Undercutting failed: {:?}", e),
    }

    // 6. Fetch profile reviews
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
                if r.stars == 5 && r.text.is_some() && let Some(order_id) = r.order_id {
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
        Err(e) => eprintln!("Failed to fetch reviews: {:?}", e),
    }

    // 7. Perform balance withdrawal (Example payout)
    let _withdraw_req = WithdrawRequest {
        currency: "rub".to_string(),
        ext_currency: "yookassa_card".to_string(),
        wallet: "4276000000000000".to_string(),
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

    tokio::time::sleep(Duration::from_secs(5)).await;

    Ok(())
}
