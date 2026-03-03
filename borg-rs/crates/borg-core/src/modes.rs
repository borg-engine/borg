use std::sync::OnceLock;

use crate::types::PipelineMode;

static MODES: OnceLock<Vec<PipelineMode>> = OnceLock::new();

/// Register all built-in modes. Must be called once at startup before any
/// `get_mode` / `all_modes` calls. Typically called from the server binary
/// with the modes provided by `borg_domains::all_modes()`.
///
/// Panics if any mode has an invalid phase transition graph (broken `next` or
/// `retry_phase` reference). Built-in modes are static — a broken reference is
/// a programming error and must be caught at startup.
pub fn register_modes(modes: Vec<PipelineMode>) {
    for mode in &modes {
        if let Err(e) = mode.validate_phase_graph() {
            panic!("invalid built-in mode: {e}");
        }
    }
    if MODES.set(modes).is_err() {
        tracing::warn!("register_modes called more than once; subsequent call ignored");
    }
}

pub fn all_modes() -> Vec<PipelineMode> {
    MODES.get().cloned().unwrap_or_default()
}

pub fn get_mode(name: &str) -> Option<PipelineMode> {
    let alias = match name {
        "swe" => "sweborg",
        "legal" => "lawborg",
        _ => name,
    };
    all_modes().into_iter().find(|m| m.name == alias)
}
