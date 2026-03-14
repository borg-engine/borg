use std::{collections::HashMap, sync::Arc};

use crate::agent::AgentBackend;

/// Central registry holding all trait implementors for plugin extensibility.
/// Wired into AppState for access from routes and pipeline.
pub struct PluginRegistry {
    pub backends: HashMap<String, Arc<dyn AgentBackend>>,
}

impl PluginRegistry {
    pub fn new(backends: HashMap<String, Arc<dyn AgentBackend>>) -> Self {
        Self { backends }
    }

    pub fn get_backend(&self, name: &str) -> Option<&Arc<dyn AgentBackend>> {
        self.backends.get(name)
    }

    pub fn default_backend(&self) -> Option<&Arc<dyn AgentBackend>> {
        self.backends
            .get("agent-sdk")
            .or_else(|| self.backends.get("claude"))
    }

    pub fn backend_names(&self) -> Vec<&str> {
        self.backends.keys().map(|s| s.as_str()).collect()
    }
}

/// Builder for constructing a PluginRegistry.
pub struct RegistryBuilder {
    backends: HashMap<String, Arc<dyn AgentBackend>>,
}

impl RegistryBuilder {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
        }
    }

    pub fn backend(mut self, name: impl Into<String>, backend: Arc<dyn AgentBackend>) -> Self {
        self.backends.insert(name.into(), backend);
        self
    }

    pub fn backends(mut self, backends: HashMap<String, Arc<dyn AgentBackend>>) -> Self {
        self.backends.extend(backends);
        self
    }

    pub fn build(self) -> PluginRegistry {
        PluginRegistry::new(self.backends)
    }
}

impl Default for RegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}
