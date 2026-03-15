use std::sync::OnceLock;

use tokio::sync::Mutex;
use tracing::{info, warn};

static LAST_UPDATE: OnceLock<Mutex<std::time::Instant>> = OnceLock::new();

const UPDATE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);

/// Ensure agent CLI tools (claude, codex) are up to date.
/// No-ops if called within 24h of the last successful update.
/// Safe to call from hot paths — the cooldown check is just a mutex + instant comparison.
pub async fn ensure_agent_tools_updated() {
    let mutex = LAST_UPDATE.get_or_init(|| Mutex::new(std::time::Instant::now() - UPDATE_INTERVAL));
    let mut last = mutex.lock().await;
    if last.elapsed() < UPDATE_INTERVAL {
        return;
    }

    info!("updating agent CLI tools (>24h since last update)");
    let cmds: &[(&str, &[&str])] = &[
        ("bun", &["install", "-g", "@anthropic-ai/claude-code@latest"]),
        ("bun", &["install", "-g", "@openai/codex@latest"]),
    ];
    for (bin, args) in cmds {
        match tokio::process::Command::new(bin)
            .args(*args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
        {
            Ok(out) if out.status.success() => {
                info!("updated: {bin} {}", args.join(" "));
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                warn!("failed to update {bin} {}: {stderr}", args.join(" "));
            }
            Err(e) => {
                warn!("failed to run {bin}: {e}");
            }
        }
    }
    *last = std::time::Instant::now();
}
