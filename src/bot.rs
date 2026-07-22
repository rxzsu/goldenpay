//! Polling bot that monitors orders and messages with state persistence.

use crate::error::GoldenPayError;
use crate::event::{BotOptions, EventStream, MessageFilter};
use crate::models::{BotState, ChatMessage, OfferEdit, OrderInfo};
use crate::scheduler::OfferScheduler;
use crate::session::SessionManager;
use crate::storage::{JsonStateStore, MemoryStateStore, StateStore};
use chrono::{Local, Timelike};
use std::sync::Arc;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// An event emitted by [`GoldenPayBot`] during a poll cycle.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum GoldenPayEvent {
    /// A previously unseen order appeared on the trade page.
    NewOrder(OrderInfo),
    /// A new message was received in an active chat.
    NewMessage(ChatMessage),
}

/// A polling bot that monitors new orders and messages.
///
/// Call [`run`](GoldenPayBot::run) to start the event loop.
/// Supports graceful shutdown via [`CancellationToken`].
pub struct GoldenPayBot {
    manager: SessionManager,
    store: Arc<dyn StateStore>,
    stream: EventStream,
    options: BotOptions,
    cancel_token: CancellationToken,
    concurrency_limit: Arc<Semaphore>,
    scheduler: Option<OfferScheduler>,
}

impl GoldenPayBot {
    /// Creates a bot from a [`SessionManager`] with auto-reconnect support.
    #[must_use]
    pub fn new(manager: SessionManager) -> Self {
        let store: Arc<dyn StateStore> = if let Some(path) = manager.config().state_path.clone() {
            Arc::new(JsonStateStore::new(path))
        } else {
            Arc::new(MemoryStateStore::new())
        };

        Self {
            manager,
            store,
            stream: EventStream::default(),
            options: BotOptions::default(),
            cancel_token: CancellationToken::new(),
            concurrency_limit: Arc::new(Semaphore::new(5)),
            scheduler: None,
        }
    }

    /// Connects to `FunPay` and creates a bot with auto-reconnect support.
    pub async fn connect(client: crate::client::GoldenPay) -> Result<Self, GoldenPayError> {
        let manager = SessionManager::connect(client).await?;
        Ok(Self::new(manager))
    }

    /// Creates a bot from a session manager with a custom state store.
    pub fn with_store(manager: SessionManager, store: Arc<dyn StateStore>) -> Self {
        Self {
            manager,
            store,
            stream: EventStream::default(),
            options: BotOptions::default(),
            cancel_token: CancellationToken::new(),
            concurrency_limit: Arc::new(Semaphore::new(5)),
            scheduler: None,
        }
    }

    /// Attaches an offer group scheduler for automatic activation/deactivation.
    #[must_use]
    pub fn with_scheduler(mut self, scheduler: OfferScheduler) -> Self {
        self.scheduler = Some(scheduler);
        self
    }

    /// Applies bot options (message filtering, etc.).
    #[must_use]
    pub fn with_options(mut self, options: BotOptions) -> Self {
        self.options = options;
        self
    }

    /// Enables auto-raising for specified category/game node IDs with an optional interval (defaults to 2 hours).
    #[must_use]
    pub fn with_auto_raise(
        mut self,
        node_ids: Vec<i64>,
        interval: Option<std::time::Duration>,
    ) -> Self {
        self.options.auto_raise_nodes = Some(node_ids);
        self.options.auto_raise_interval = interval;
        self
    }

    /// Sets a welcome message to be automatically sent to the chat when a new order is received.
    #[must_use]
    pub fn with_welcome_message(mut self, message: impl Into<String>) -> Self {
        self.options.auto_welcome_message = Some(message.into());
        self
    }

    /// Configures the bot's sleep schedule to activate/deactivate specified offers during configured hours.
    #[must_use]
    pub fn with_sleep_schedule(
        mut self,
        start_hour: u32,
        end_hour: u32,
        node_offers: Vec<(i64, i64)>,
    ) -> Self {
        self.options.sleep_start_hour = Some(start_hour);
        self.options.sleep_end_hour = Some(end_hour);
        self.options.sleep_node_offers = Some(node_offers);
        self
    }

    /// Associates a cancellation token for graceful shutdown.
    /// When cancelled, the bot's [`run`](GoldenPayBot::run) loop exits cleanly.
    #[must_use]
    pub fn with_cancellation_token(mut self, token: CancellationToken) -> Self {
        self.cancel_token = token;
        self
    }

    /// Sets the maximum number of concurrent API requests (default: 5).
    #[must_use]
    pub fn with_concurrency_limit(mut self, max_concurrent: usize) -> Self {
        self.concurrency_limit = Arc::new(Semaphore::new(max_concurrent));
        self
    }

    /// Triggers a graceful shutdown of the bot.
    pub fn cancel(&self) {
        self.cancel_token.cancel();
    }

    /// Spawns a task that listens for Ctrl+C and triggers graceful shutdown.
    pub fn listen_for_shutdown(&self) {
        let token = self.cancel_token.clone();
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            tracing::info!("received Ctrl+C, shutting down");
            token.cancel();
        });
    }

    /// Returns a reference to the session manager.
    #[must_use]
    pub fn manager(&self) -> &SessionManager {
        &self.manager
    }

    /// Returns a mutable reference to the session manager.
    pub fn manager_mut(&mut self) -> &mut SessionManager {
        &mut self.manager
    }

    /// Returns a reference to the underlying authenticated session.
    #[must_use]
    pub fn session(&self) -> &crate::client::GoldenPaySession {
        self.manager.session()
    }

    /// Loads previously persisted state (seen orders and messages) from the store.
    pub async fn load_state(&mut self) -> Result<(), GoldenPayError> {
        let state = self.store.load().await?;
        self.stream.seen_orders = state.seen_orders.into_iter().collect();
        self.stream.seen_messages = state.seen_messages;
        tracing::info!(orders = %self.stream.seen_orders.len(), "state loaded");
        Ok(())
    }

    /// Persists current state (seen orders and messages) to the store.
    pub async fn save_state(&self) -> Result<(), GoldenPayError> {
        self.store
            .save(&BotState {
                seen_orders: self.stream.seen_orders.iter().cloned().collect(),
                seen_messages: self.stream.seen_messages.clone(),
            })
            .await
    }

    /// Pre-populates seen orders and messages so that past events are not re-emitted.
    pub async fn bootstrap(&mut self) -> Result<(), GoldenPayError> {
        let orders = self.manager.fetch_orders().await?;
        tracing::info!(count = %orders.len(), "bootstrapping existing orders");

        for order in &orders {
            self.stream.seen_orders.insert(order.id.clone());
        }

        let session = self.manager.session().clone();
        let sem = self.concurrency_limit.clone();
        let mut set = JoinSet::new();
        for order in &orders {
            let chat_id = order.chat_id.clone();
            let session = session.clone();
            let sem = sem.clone();
            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let messages = session.fetch_chat_messages(&chat_id).await?;
                Ok::<_, GoldenPayError>((chat_id, messages))
            });
        }

        while let Some(joined) = set.join_next().await {
            let Ok(Ok((chat_id, messages))) = joined else {
                continue;
            };
            let last_message_id = messages.iter().map(|m| m.id).max().unwrap_or_default();
            if last_message_id > 0 {
                self.stream.seen_messages.insert(chat_id, last_message_id);
            }
        }

        tracing::info!("bootstrap complete");
        self.save_state().await
    }

    /// Runs a single poll cycle: fetches orders, checks for new messages, returns events.
    pub async fn poll_once(&mut self) -> Result<Vec<GoldenPayEvent>, GoldenPayError> {
        let orders = self.manager.fetch_orders().await?;
        let mut events = Vec::new();
        let filter = MessageFilter {
            ignore_author_id: self
                .options
                .ignore_own_messages
                .then_some(self.manager.user().id),
        };

        let mut emit_chats = Vec::new();
        let mut mark_chats = Vec::new();
        for order in &orders {
            let chat_id = order.chat_id.clone();
            let is_new_order = self.stream.should_emit_order(order);

            if is_new_order {
                events.push(GoldenPayEvent::NewOrder(order.clone()));
            }

            let should_emit = self.options.emit_messages_for_new_orders || !is_new_order;
            if should_emit {
                emit_chats.push(chat_id);
            } else {
                mark_chats.push(chat_id);
            }
        }

        let session = self.manager.session().clone();
        let sem = self.concurrency_limit.clone();
        let mut set = JoinSet::new();
        for chat_id in &emit_chats {
            let session = session.clone();
            let cid = chat_id.clone();
            let sem = sem.clone();
            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let r = session.fetch_chat_messages(&cid).await;
                (cid, r, true)
            });
        }
        for chat_id in &mark_chats {
            let session = session.clone();
            let cid = chat_id.clone();
            let sem = sem.clone();
            set.spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let r = session.fetch_chat_messages(&cid).await;
                (cid, r, false)
            });
        }

        while let Some(joined) = set.join_next().await {
            let Ok((chat_id, result, should_emit)) = joined else {
                continue;
            };
            let Ok(messages) = result else {
                continue;
            };
            if should_emit {
                for message in &messages {
                    if self.stream.should_emit_message(message, &filter) {
                        events.push(GoldenPayEvent::NewMessage(message.clone()));
                    }
                }
            } else {
                let last_seen = messages.iter().map(|m| m.id).max().unwrap_or_default();
                if last_seen > 0 {
                    self.stream.seen_messages.insert(chat_id, last_seen);
                }
            }
        }

        self.save_state().await?;
        tracing::debug!(events = %events.len(), "poll cycle complete");
        Ok(events)
    }

    pub async fn run<F, Fut>(&mut self, mut handler: F) -> Result<(), GoldenPayError>
    where
        F: FnMut(GoldenPayEvent, &crate::client::GoldenPaySession) -> Fut,
        Fut: std::future::Future<Output = Result<(), GoldenPayError>>,
    {
        tracing::info!("bot started");
        let token = self.cancel_token.clone();
        let auto_raise_interval = self
            .options
            .auto_raise_interval
            .unwrap_or(std::time::Duration::from_secs(7200));
        let mut last_raise = tokio::time::Instant::now() - auto_raise_interval;
        let mut currently_sleeping = None;

        loop {
            tokio::select! {
                () = token.cancelled() => {
                    tracing::info!("bot received shutdown signal");
                    return Ok(());
                }
                result = self.poll_once() => {
                    let events = result?;
                    for event in events {
                        if let GoldenPayEvent::NewOrder(ref order) = event
                            && let Some(ref welcome_msg) = self.options.auto_welcome_message {
                                tracing::info!(order_id = %order.id, "sending auto-welcome message");
                                if let Err(e) = self.manager.send_message(&order.chat_id, welcome_msg).await {
                                    tracing::error!(order_id = %order.id, error = %e, "failed to send auto-welcome message");
                                }
                            }
                        handler(event, self.manager.session()).await?;
                    }

                    // Handle sleep schedule if configured
                    if let (Some(start), Some(end), Some(offers)) = (
                        self.options.sleep_start_hour,
                        self.options.sleep_end_hour,
                        &self.options.sleep_node_offers,
                    ) {
                        let hour = Local::now().hour();
                        let should_sleep = if start <= end {
                            hour >= start && hour < end
                        } else {
                            hour >= start || hour < end
                        };

                        if currently_sleeping != Some(should_sleep) {
                            tracing::info!(should_sleep, "transitioning sleep state");
                            for &(node_id, offer_id) in offers {
                                let patch = OfferEdit {
                                    active: Some(!should_sleep),
                                    ..Default::default()
                                };
                                match self.manager.edit_offer(node_id, offer_id, patch).await {
                                    Ok(_) => {
                                        tracing::info!(node_id, offer_id, active = !should_sleep, "updated offer state according to sleep schedule");
                                    }
                                    Err(e) => {
                                        tracing::error!(node_id, offer_id, error = %e, "failed to update offer state for sleep schedule");
                                    }
                                }
                            }
                            currently_sleeping = Some(should_sleep);
                        }
                    }

                    // Evaluate offer group scheduler
                    if let Some(ref mut scheduler) = self.scheduler {
                        let transitions = scheduler.poll();
                        for (entry, should_be_active) in &transitions {
                            let node_id = entry.group.node_id();
                            let offers = match self.manager.fetch_my_offers(node_id).await {
                                Ok(o) => o,
                                Err(e) => {
                                    tracing::error!(node_id, entry = %entry.name, error = %e, "scheduler: failed to fetch offers");
                                    continue;
                                }
                            };
                            for offer in &offers {
                                if entry.group.active_only() && !offer.active {
                                    continue;
                                }
                                if offer.active == *should_be_active {
                                    continue;
                                }
                                let patch = OfferEdit {
                                    active: Some(*should_be_active),
                                    ..Default::default()
                                };
                                if let Err(e) = self.manager.edit_offer(node_id, offer.id, patch).await {
                                    tracing::error!(offer_id = offer.id, entry = %entry.name, error = %e, "scheduler: failed to update offer");
                                }
                            }
                        }
                    }

                    // Handle auto-raising if configured
                    if let Some(nodes) = &self.options.auto_raise_nodes {
                        let now = tokio::time::Instant::now();
                        if now.duration_since(last_raise) >= auto_raise_interval {
                            for node_id in nodes {
                                match self.manager.raise_offers(*node_id).await {
                                    Ok(res) => {
                                        tracing::info!(node_id, success = res.success, message = ?res.message, "auto-raised offers");
                                    }
                                    Err(e) => {
                                        tracing::error!(node_id, error = %e, "failed to auto-raise offers");
                                    }
                                }
                            }
                            last_raise = now;
                        }
                    }

                    tokio::time::sleep(self.manager.poll_interval()).await;
                }
            }
        }
    }
}
