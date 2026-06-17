use crate::models::{ChatMessage, OrderInfo};
use std::collections::{HashMap, HashSet};

/// Configuration options for [`GoldenPayBot`](crate::GoldenPayBot).
#[derive(Debug, Clone)]
pub struct BotOptions {
    /// If true, messages authored by the bot's user are not emitted.
    pub ignore_own_messages: bool,
    /// If true, new orders also emit their initial chat messages.
    pub emit_messages_for_new_orders: bool,
}

impl Default for BotOptions {
    fn default() -> Self {
        Self {
            ignore_own_messages: true,
            emit_messages_for_new_orders: true,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    pub ignore_author_id: Option<i64>,
}

#[derive(Debug, Clone, Default)]
pub struct EventStream {
    pub seen_orders: HashSet<String>,
    pub seen_messages: HashMap<String, i64>,
}

impl EventStream {
    pub fn should_emit_order(&mut self, order: &OrderInfo) -> bool {
        self.seen_orders.insert(order.id.clone())
    }

    pub fn should_emit_message(&mut self, message: &ChatMessage, filter: &MessageFilter) -> bool {
        if filter.ignore_author_id == Some(message.author_id) {
            return false;
        }

        let last_seen = self
            .seen_messages
            .get(&message.chat_id)
            .copied()
            .unwrap_or_default();
        if message.id <= last_seen {
            return false;
        }

        self.seen_messages
            .insert(message.chat_id.clone(), message.id);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ChatMessage, OrderInfo, OrderStatus};

    #[test]
    fn emits_order_only_once() {
        let mut stream = EventStream::default();
        let order = OrderInfo {
            id: "ORDER1".to_string(),
            buyer_username: "buyer".to_string(),
            buyer_id: 2,
            chat_id: "users-1-2".to_string(),
            description: "desc".to_string(),
            subcategory_name: "Steam".to_string(),
            amount: 1,
            status: OrderStatus::Paid,
        };

        assert!(stream.should_emit_order(&order));
        assert!(!stream.should_emit_order(&order));
    }

    #[test]
    fn filters_own_messages_and_dedups() {
        let mut stream = EventStream::default();
        let filter = MessageFilter {
            ignore_author_id: Some(1),
        };

        let own = ChatMessage {
            id: 1,
            chat_id: "users-1-2".to_string(),
            author_id: 1,
            text: Some("hi".to_string()),
        };
        let incoming = ChatMessage {
            id: 2,
            chat_id: "users-1-2".to_string(),
            author_id: 2,
            text: Some("yo".to_string()),
        };

        assert!(!stream.should_emit_message(&own, &filter));
        assert!(stream.should_emit_message(&incoming, &filter));
        assert!(!stream.should_emit_message(&incoming, &filter));
    }
}
