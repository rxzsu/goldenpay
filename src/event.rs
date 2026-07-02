//! Event stream for deduplicated order and message emission.

use crate::models::{ChatMessage, OrderInfo};
use std::collections::{HashMap, HashSet};

/// Configuration options for [`GoldenPayBot`](crate::GoldenPayBot).
#[derive(Debug, Clone)]
pub struct BotOptions {
    /// If true, messages authored by the bot's user are not emitted.
    pub ignore_own_messages: bool,
    /// If true, new orders also emit their initial chat messages.
    pub emit_messages_for_new_orders: bool,
    /// Category node IDs to automatically raise.
    pub auto_raise_nodes: Option<Vec<i64>>,
    /// Interval for auto-raising offers (defaults to 2 hours if None and nodes are set).
    pub auto_raise_interval: Option<std::time::Duration>,
    /// Welcome message to automatically send when a new order is received.
    pub auto_welcome_message: Option<String>,
}

impl Default for BotOptions {
    fn default() -> Self {
        Self {
            ignore_own_messages: true,
            emit_messages_for_new_orders: true,
            auto_raise_nodes: None,
            auto_raise_interval: None,
            auto_welcome_message: None,
        }
    }
}

/// Filter for suppressing messages by author.
#[derive(Debug, Clone, Default)]
pub struct MessageFilter {
    pub ignore_author_id: Option<i64>,
}

/// Tracks already-seen orders and messages to avoid duplicate events.
#[derive(Debug, Clone, Default)]
pub struct EventStream {
    pub seen_orders: HashSet<String>,
    pub seen_messages: HashMap<String, i64>,
}

impl EventStream {
    /// Returns `true` if this order is new (inserts it into the seen set).
    pub fn should_emit_order(&mut self, order: &OrderInfo) -> bool {
        self.seen_orders.insert(order.id.clone())
    }

    /// Returns `true` if this message is new and passes the given filter.
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
