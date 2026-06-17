use crate::error::GoldenPayError;
use crate::event::{BotOptions, EventStream, MessageFilter};
use crate::models::{BotState, ChatMessage, OrderInfo};
use crate::session::SessionManager;
use crate::storage::{JsonStateStore, MemoryStateStore, StateStore};
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
/// Supports graceful shutdown via [`CancellationToken`](tokio_util::sync::CancellationToken).
pub struct GoldenPayBot {
    manager: SessionManager,
    store: Arc<dyn StateStore>,
    stream: EventStream,
    options: BotOptions,
    cancel_token: CancellationToken,
    concurrency_limit: Arc<Semaphore>,
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
        }
    }

    /// Connects to `FunPay` and creates a bot with auto-reconnect support.
    pub async fn connect(client: crate::client::GoldenPay) -> Result<Self, GoldenPayError> {
        let manager = SessionManager::connect(client).await?;
        Ok(Self::new(manager))
    }

    pub fn with_store(manager: SessionManager, store: Arc<dyn StateStore>) -> Self {
        Self {
            manager,
            store,
            stream: EventStream::default(),
            options: BotOptions::default(),
            cancel_token: CancellationToken::new(),
            concurrency_limit: Arc::new(Semaphore::new(5)),
        }
    }

    #[must_use]
    pub fn with_options(mut self, options: BotOptions) -> Self {
        self.options = options;
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

    pub async fn load_state(&mut self) -> Result<(), GoldenPayError> {
        let state = self.store.load().await?;
        self.stream.seen_orders = state.seen_orders.into_iter().collect();
        self.stream.seen_messages = state.seen_messages;
        tracing::info!(orders = %self.stream.seen_orders.len(), "state loaded");
        Ok(())
    }

    pub async fn save_state(&self) -> Result<(), GoldenPayError> {
        self.store
            .save(&BotState {
                seen_orders: self.stream.seen_orders.iter().cloned().collect(),
                seen_messages: self.stream.seen_messages.clone(),
            })
            .await
    }

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
            let Ok(Ok((chat_id, messages))) = joined else { continue; };
            let last_message_id = messages.iter().map(|m| m.id).max().unwrap_or_default();
            if last_message_id > 0 {
                self.stream.seen_messages.insert(chat_id, last_message_id);
            }
        }

        tracing::info!("bootstrap complete");
        self.save_state().await
    }

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
            let is_new_order = self.stream.seen_orders.insert(order.id.clone());

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
            let Ok((chat_id, result, should_emit)) = joined else { continue; };
            let Ok(messages) = result else { continue; };
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
        loop {
            tokio::select! {
                () = token.cancelled() => {
                    tracing::info!("bot received shutdown signal");
                    return Ok(());
                }
                result = self.poll_once() => {
                    let events = result?;
                    for event in events {
                        handler(event, self.manager.session()).await?;
                    }
                    tokio::time::sleep(self.manager.poll_interval()).await;
                }
            }
        }
    }
}
