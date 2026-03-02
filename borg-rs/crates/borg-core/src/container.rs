use std::{
    collections::HashMap,
    sync::Arc,
    time::SystemTime,
};

use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub container_id: String,
    pub image: String,
    pub started_at: SystemTime,
    pub phase: String,
}

/// Per-task container ID registry for live inspection and post-mortem.
pub struct ContainerRegistry {
    entries: Mutex<HashMap<i64, ContainerInfo>>,
}

impl ContainerRegistry {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::new()),
        })
    }

    pub async fn register(&self, task_id: i64, info: ContainerInfo) {
        self.entries.lock().await.insert(task_id, info);
    }

    pub async fn remove(&self, task_id: i64) {
        self.entries.lock().await.remove(&task_id);
    }

    pub async fn get(&self, task_id: i64) -> Option<ContainerInfo> {
        self.entries.lock().await.get(&task_id).cloned()
    }

    pub async fn list(&self) -> Vec<(i64, ContainerInfo)> {
        self.entries
            .lock()
            .await
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }
}
