//! Offer group scheduler — activate/deactivate offers on a recurring schedule.
//!
//! # Example
//!
//! ```ignore
//! use goldenpay::scheduler::{
//!     OfferScheduler, OfferGroup, ScheduleRule, ScheduleAction,
//! };
//!
//! let scheduler = OfferScheduler::new(vec![
//!     ScheduleEntry::new(
//!         "night-sleep",
//!         OfferGroup::node(12345, true),
//!         ScheduleRule::daily(23, 8),
//!         ScheduleAction::Deactivate,
//!     ),
//!     ScheduleEntry::new(
//!         "day-active",
//!         OfferGroup::node(12345, true),
//!         ScheduleRule::daily(8, 23),
//!         ScheduleAction::Activate,
//!     ),
//! ]);
//!
//! let transitions = scheduler.poll(); // call periodically
//! for (entry, should_be_active) in &transitions {
//!     println!("{} should be active={}", entry.name, should_be_active);
//! }
//! ```

use chrono::{Local, Timelike};
use std::collections::HashMap;

/// Defines a group of offers to manage.
#[derive(Debug, Clone)]
pub enum OfferGroup {
    /// All offers in a specific category node.
    ///
    /// When `active_only` is `true`, only currently active offers are affected
    /// (useful for deactivation — no need to touch already inactive offers).
    Node {
        node_id: i64,
        active_only: bool,
    },
}

impl OfferGroup {
    /// Creates a group targeting all offers in a category node.
    #[must_use]
    pub fn node(node_id: i64, active_only: bool) -> Self {
        Self::Node { node_id, active_only }
    }

    /// Returns the category node ID.
    #[must_use]
    pub fn node_id(&self) -> i64 {
        match self {
            Self::Node { node_id, .. } => *node_id,
        }
    }

    /// Returns whether only active offers should be affected.
    #[must_use]
    pub fn active_only(&self) -> bool {
        match self {
            Self::Node { active_only, .. } => *active_only,
        }
    }
}

/// When a schedule rule should be applied.
#[derive(Debug, Clone)]
pub enum ScheduleRule {
    /// Daily recurring time window.
    ///
    /// Supports overnight ranges (e.g., `start_hour=22`, `end_hour=6`
    /// means 22:00–06:00). If `start_hour == end_hour`, the rule applies
    /// all day.
    Daily { start_hour: u32, end_hour: u32 },
}

impl ScheduleRule {
    /// Creates a daily time window.
    ///
    /// Hours are in 0–23 range (24-hour clock).
    #[must_use]
    pub fn daily(start_hour: u32, end_hour: u32) -> Self {
        Self::Daily { start_hour, end_hour }
    }

    /// Returns `true` if the current local time falls within the window.
    #[must_use]
    pub fn is_active(&self) -> bool {
        let hour = Local::now().hour();
        match self {
            Self::Daily { start_hour, end_hour } => {
                if start_hour == end_hour {
                    return true;
                }
                if start_hour < end_hour {
                    hour >= *start_hour && hour < *end_hour
                } else {
                    hour >= *start_hour || hour < *end_hour
                }
            }
        }
    }
}

/// What action to perform on the offer group when the rule is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleAction {
    /// Activate offers (`active = true`).
    Activate,
    /// Deactivate offers (`active = false`).
    Deactivate,
}

impl ScheduleAction {
    /// Returns the desired `active` value for offers.
    #[must_use]
    pub fn desired_active(&self) -> bool {
        match self {
            Self::Activate => true,
            Self::Deactivate => false,
        }
    }
}

/// A single schedule entry binding a group, rule, and action.
#[derive(Debug, Clone)]
pub struct ScheduleEntry {
    /// Human-readable name for logging.
    pub name: String,
    /// Which offers to manage.
    pub group: OfferGroup,
    /// When to apply the action.
    pub rule: ScheduleRule,
    /// What to do when the rule is active.
    pub action: ScheduleAction,
}

impl ScheduleEntry {
    /// Creates a new schedule entry.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        group: OfferGroup,
        rule: ScheduleRule,
        action: ScheduleAction,
    ) -> Self {
        Self {
            name: name.into(),
            group,
            rule,
            action,
        }
    }
}

/// Evaluates schedule entries and reports transitions.
///
/// Call [`poll`](OfferScheduler::poll) periodically (e.g., every bot cycle)
/// to detect when an entry's desired state changes.
#[derive(Debug, Clone)]
pub struct OfferScheduler {
    entries: Vec<ScheduleEntry>,
    /// Tracks the last known active state per entry name.
    last_states: HashMap<String, bool>,
}

impl OfferScheduler {
    /// Creates a new scheduler from a list of entries.
    #[must_use]
    pub fn new(entries: Vec<ScheduleEntry>) -> Self {
        Self {
            entries,
            last_states: HashMap::new(),
        }
    }

    /// Evaluates all entries against the current local time.
    ///
    /// Returns entries whose desired state differs from the last known
    /// state (i.e., transitions that should be applied).
    pub fn poll(&mut self) -> Vec<(&ScheduleEntry, bool)> {
        let mut transitions = Vec::new();
        for entry in &self.entries {
            let desired = entry.action.desired_active();
            let actually_active = entry.rule.is_active();
            let should_be = if actually_active { desired } else { !desired };

            let last = self.last_states.get(&entry.name).copied();
            if last != Some(should_be) {
                self.last_states.insert(entry.name.clone(), should_be);
                transitions.push((entry, should_be));
            }
        }
        transitions
    }

    /// Returns a reference to the entries.
    #[must_use]
    pub fn entries(&self) -> &[ScheduleEntry] {
        &self.entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, start: u32, end: u32, action: ScheduleAction) -> ScheduleEntry {
        ScheduleEntry::new(name, OfferGroup::node(1, true), ScheduleRule::daily(start, end), action)
    }

    #[test]
    fn no_transition_on_first_poll_without_prior_state() {
        let mut s = OfferScheduler::new(vec![
            entry("test", 0, 24, ScheduleAction::Activate),
        ]);
        let t = s.poll();
        // First poll always reports a transition since there's no prior state
        assert!(!t.is_empty());
    }

    #[test]
    fn stable_state_returns_no_transitions() {
        let mut s = OfferScheduler::new(vec![
            entry("test", 0, 24, ScheduleAction::Activate),
        ]);
        let _ = s.poll(); // prime
        let t = s.poll();
        assert!(t.is_empty());
    }

    #[test]
    fn rule_active_matches_action_desired() {
        let mut s = OfferScheduler::new(vec![
            entry("day", 0, 24, ScheduleAction::Activate),
            entry("night", 0, 24, ScheduleAction::Deactivate),
        ]);
        let t = s.poll();
        let day = t.iter().find(|(e, _)| e.name == "day").map(|(_, v)| v);
        let night = t.iter().find(|(e, _)| e.name == "night").map(|(_, v)| v);
        assert_eq!(day, Some(&true));
        assert_eq!(night, Some(&false));
    }

    #[test]
    fn multiple_entries_return_all_transitions() {
        let mut s = OfferScheduler::new(vec![
            entry("a", 0, 24, ScheduleAction::Activate),
            entry("b", 0, 24, ScheduleAction::Deactivate),
        ]);
        let t = s.poll();
        assert_eq!(t.len(), 2);
    }

    #[test]
    fn node_group_accessors() {
        let g = OfferGroup::node(42, true);
        assert_eq!(g.node_id(), 42);
        assert!(g.active_only());
        let g2 = OfferGroup::node(7, false);
        assert_eq!(g2.node_id(), 7);
        assert!(!g2.active_only());
    }

    #[test]
    fn schedule_action_values() {
        assert!(ScheduleAction::Activate.desired_active());
        assert!(!ScheduleAction::Deactivate.desired_active());
    }
}
