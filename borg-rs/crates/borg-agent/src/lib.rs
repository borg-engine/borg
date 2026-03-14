pub mod bridge;
pub mod claude;
pub mod container;
pub use claude::extract_phase_result;
pub mod codex;
pub(crate) mod drain;
pub mod event;
pub mod gemini;
pub mod instruction;
pub mod mcp;
pub mod ollama;
pub mod reliable;

pub use bridge::AgentSdkBackend;
pub use gemini::GeminiBackend;
pub use ollama::OllamaBackend;
pub use reliable::ReliableBackend;
