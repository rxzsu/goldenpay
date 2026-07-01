//! State persistence adapters for bot state (seen orders and messages).

use crate::error::GoldenPayError;
use crate::models::BotState;
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::Mutex;

/// Persistence adapter for bot state (seen orders and messages).
#[async_trait]
pub trait StateStore: Send + Sync {
    async fn load(&self) -> Result<BotState, GoldenPayError>;
    async fn save(&self, state: &BotState) -> Result<(), GoldenPayError>;
}

/// In-memory bot state store (no persistence across restarts).
#[derive(Default)]
pub struct MemoryStateStore {
    state: Arc<Mutex<BotState>>,
}

impl MemoryStateStore {
    /// Creates an empty in-memory state store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl StateStore for MemoryStateStore {
    async fn load(&self) -> Result<BotState, GoldenPayError> {
        Ok(self.state.lock().await.clone())
    }

    async fn save(&self, state: &BotState) -> Result<(), GoldenPayError> {
        *self.state.lock().await = state.clone();
        Ok(())
    }
}

/// JSON-file-backed bot state store with atomic writes.
pub struct JsonStateStore {
    path: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl JsonStateStore {
    /// Creates a store that persists bot state to the given file path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            lock: Arc::new(Mutex::new(())),
        }
    }
}

#[async_trait]
impl StateStore for JsonStateStore {
    async fn load(&self) -> Result<BotState, GoldenPayError> {
        let _guard = self.lock.lock().await;
        if !self.path.exists() {
            return Ok(BotState::default());
        }

        let raw = fs::read_to_string(&self.path).await?;
        Ok(serde_json::from_str(&raw)?)
    }

    async fn save(&self, state: &BotState) -> Result<(), GoldenPayError> {
        let _guard = self.lock.lock().await;
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let raw = serde_json::to_string_pretty(state)?;
        write_atomic_json(&self.path, &raw).await?;
        Ok(())
    }
}

async fn write_atomic_json(path: &std::path::Path, raw: &str) -> Result<(), GoldenPayError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| GoldenPayError::state(format!("invalid file name for {}", path.display())))?;
    let tmp_path = path.with_file_name(format!("{file_name}.tmp"));

    fs::write(&tmp_path, raw).await?;
    fs::rename(&tmp_path, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("goldenpay-{name}-{stamp}.json"))
    }

    #[tokio::test]
    async fn json_store_roundtrip() {
        let path = temp_path("state");
        let store = JsonStateStore::new(&path);

        let mut state = BotState::default();
        state.seen_orders.push("ORDER123".to_string());
        state.seen_messages.insert("users-1-2".to_string(), 42);

        store.save(&state).await.unwrap();
        let loaded = store.load().await.unwrap();

        assert_eq!(loaded.seen_orders, vec!["ORDER123".to_string()]);
        assert_eq!(loaded.seen_messages.get("users-1-2"), Some(&42));

        let _ = fs::remove_file(path).await;
    }
}
