use crate::client::GoldenPaySession;
use crate::error::GoldenPayError;
use crate::event::{BotOptions, EventStream, MessageFilter};
use crate::models::{BotState, ChatMessage, OrderInfo};
use crate::storage::{JsonStateStore, MemoryStateStore, StateStore};
use std::sync::Arc;
use tokio::task::JoinSet;

#[derive(Debug, Clone)]
pub enum GoldenPayEvent {
    NewOrder(OrderInfo),
    NewMessage(ChatMessage),
}

pub struct GoldenPayBot {
    session: GoldenPaySession,
    store: Arc<dyn StateStore>,
    stream: EventStream,
    options: BotOptions,
}

impl GoldenPayBot {
    pub fn new(session: GoldenPaySession) -> Self {
        let store: Arc<dyn StateStore> = if let Some(path) = session.config().state_path.clone() {
            Arc::new(JsonStateStore::new(path))
        } else {
            Arc::new(MemoryStateStore::new())
        };

        Self {
            session,
            store,
            stream: EventStream::default(),
            options: BotOptions::default(),
        }
    }

    pub fn with_store(session: GoldenPaySession, store: Arc<dyn StateStore>) -> Self {
        Self {
            session,
            store,
            stream: EventStream::default(),
            options: BotOptions::default(),
        }
    }

    pub fn with_options(mut self, options: BotOptions) -> Self {
        self.options = options;
        self
    }

    pub fn session(&self) -> &GoldenPaySession {
        &self.session
    }

    pub async fn load_state(&mut self) -> Result<(), GoldenPayError> {
        let state = self.store.load().await?;
        self.stream.seen_orders = state.seen_orders.into_iter().collect();
        self.stream.seen_messages = state.seen_messages;
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
        let orders = self.session.fetch_orders().await?;

        for order in &orders {
            self.stream.seen_orders.insert(order.id.clone());
        }

        let session = self.session.clone();
        let mut set = JoinSet::new();
        for order in &orders {
            let chat_id = order.chat_id.clone();
            let session = session.clone();
            set.spawn(async move {
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

        self.save_state().await
    }

    pub async fn poll_once(&mut self) -> Result<Vec<GoldenPayEvent>, GoldenPayError> {
        let orders = self.session.fetch_orders().await?;
        let mut events = Vec::new();
        let filter = MessageFilter {
            ignore_author_id: self
                .options
                .ignore_own_messages
                .then_some(self.session.user().id),
        };

        // Phase 1: classify orders — identify new orders and determine which chats need messages
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

        // Phase 2: fetch messages concurrently for all needed chats
        let session = self.session.clone();
        let mut set = JoinSet::new();
        for chat_id in &emit_chats {
            let session = session.clone();
            let cid = chat_id.clone();
            set.spawn(async move {
                let r = session.fetch_chat_messages(&cid).await;
                (cid, r, true)
            });
        }
        for chat_id in &mark_chats {
            let session = session.clone();
            let cid = chat_id.clone();
            set.spawn(async move {
                let r = session.fetch_chat_messages(&cid).await;
                (cid, r, false)
            });
        }

        // Phase 3: process results as they complete
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
        Ok(events)
    }

    pub async fn run<F, Fut>(&mut self, mut handler: F) -> Result<(), GoldenPayError>
    where
        F: FnMut(GoldenPayEvent, &GoldenPaySession) -> Fut,
        Fut: std::future::Future<Output = Result<(), GoldenPayError>>,
    {
        loop {
            for event in self.poll_once().await? {
                handler(event, &self.session).await?;
            }

            tokio::time::sleep(self.session.poll_interval()).await;
        }
    }
}
