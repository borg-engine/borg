//! Process sandbox for pipeline agent phases.
//!
//! Supports two isolation backends (preferred order when `auto`):
//! 1. **bwrap** — bubblewrap-based namespace isolation (no daemon, no image).
//!    Adapted from OpenAI Codex linux-sandbox. Mounts the host filesystem
//!    read-only with selective read-write bind mounts for working dirs.
//! 2. **docker** — Docker container via `docker run`.
//!
//! Set `SANDBOX_BACKEND=auto|bwrap|docker|none` in the environment.
//! Default is `auto` (bwrap if available, else docker, else direct).

use std::{path::Path, process::Stdio};

use tokio::process::Command;
use tracing::{info, warn};

/// Which sandboxing backend to use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SandboxMode {
    /// bwrap (bubblewrap) namespace sandbox — lightweight, no daemon required.
    Bwrap,
    /// Docker container — requires daemon and a pre-built image.
    Docker,
    /// No sandboxing — run the process directly on the host.
    Direct,
}

impl SandboxMode {
    /// Parse from env/config string. Unknown values fall back to `Auto`
    /// detection logic.
    pub fn from_str_or_auto(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "bwrap" => Some(Self::Bwrap),
            "docker" => Some(Self::Docker),
            "none" | "direct" => Some(Self::Direct),
            _ => None, // "auto" or unrecognised → detect at runtime
        }
    }
}

pub struct Sandbox;

impl Sandbox {
    /// Detect the best available sandbox mode given a preference string.
    ///
    /// Preference order when `preferred` is `"auto"` or empty:
    /// bwrap → docker → direct.
    pub async fn detect(preferred: &str) -> SandboxMode {
        if let Some(forced) = SandboxMode::from_str_or_auto(preferred) {
            return forced;
        }
        // auto
        if Self::bwrap_available().await {
            info!("sandbox: bwrap detected, using namespace sandbox");
            SandboxMode::Bwrap
        } else if Self::docker_available().await {
            info!("sandbox: bwrap not found, falling back to docker");
            SandboxMode::Docker
        } else {
            warn!("sandbox: neither bwrap nor docker available, running agents directly (no isolation)");
            SandboxMode::Direct
        }
    }

    pub async fn bwrap_available() -> bool {
        // bwrap relies on Linux namespaces; skip detection on other platforms
        if cfg!(not(target_os = "linux")) {
            return false;
        }
        Command::new("bwrap")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub async fn docker_available() -> bool {
        Command::new("docker")
            .arg("version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }

    // --- bwrap backend ---

    /// Build bwrap argument list for `command`.
    ///
    /// Mount order (mirrors OpenAI Codex linux-sandbox/bwrap.rs):
    /// 1. `--ro-bind / /`    — read-only root filesystem
    /// 2. `--dev /dev`       — minimal device tree (null, random, urandom, tty)
    /// 3. `--bind X X`       — per writable_dir (worktree, session dir)
    /// 4. `--bind /tmp /tmp` — shared /tmp (needed by compilers, git, etc.)
    /// 5. `--unshare-pid`    — isolated PID namespace
    /// 6. `--new-session`    — setsid (detach terminal)
    /// 7. `--die-with-parent`— auto-cleanup
    /// 8. `--proc /proc`     — fresh procfs for PID namespace
    /// 9. `--chdir`          — working directory inside sandbox
    /// 10. `--`              — command separator
    pub fn bwrap_args(
        writable_dirs: &[&str],
        working_dir: &str,
        command: &[String],
    ) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();

        args.extend(["--ro-bind", "/", "/", "--dev", "/dev"].map(str::to_string));

        for dir in writable_dirs {
            if !Path::new(dir).exists() {
                warn!("sandbox: skipping non-existent writable dir: {dir}");
                continue;
            }
            args.extend(["--bind", dir, dir].map(str::to_string));
        }

        args.extend(["--bind", "/tmp", "/tmp"].map(str::to_string));

        args.extend(
            [
                "--unshare-pid",
                "--new-session",
                "--die-with-parent",
                "--proc",
                "/proc",
            ]
            .map(str::to_string),
        );

        args.extend(["--chdir", working_dir].map(str::to_string));

        args.push("--".into());
        args.extend_from_slice(command);

        args
    }

    /// Return a `Command` that runs `command` inside a bwrap sandbox.
    ///
    /// Env vars set on the returned `Command` are inherited by the sandboxed
    /// process (bwrap passes them through by default).
    pub fn bwrap_command(writable_dirs: &[&str], working_dir: &str, command: &[String]) -> Command {
        let args = Self::bwrap_args(writable_dirs, working_dir, command);
        let mut cmd = Command::new("bwrap");
        cmd.args(args);
        cmd
    }

    // --- docker backend ---

    /// Return a `Command` that runs inside a Docker container.
    ///
    /// `binds`: `(host_path, container_path, read_only)` — bind mounts.
    /// `volumes`: `(volume_name, container_path)` — named Docker volumes.
    /// `env_vars`: passed as `-e KEY=VALUE` pairs.
    /// `working_dir`: container working directory; skipped if empty.
    /// `command`: appended after the image name (empty = use entrypoint default).
    pub fn docker_command(
        image: &str,
        binds: &[(&str, &str, bool)],
        volumes: &[(&str, &str)],
        working_dir: &str,
        command: &[String],
        env_vars: &[(&str, &str)],
    ) -> Command {
        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "-i".to_string(),
            "--pids-limit".to_string(),
            "256".to_string(),
        ];

        // Linux-only security hardening and host networking
        if cfg!(target_os = "linux") {
            args.extend([
                "--security-opt", "no-new-privileges:true",
                "--cap-drop", "ALL",
                "--network", "host",
            ].map(str::to_string));
        }

        for (host, container, ro) in binds {
            args.push("-v".to_string());
            if *ro {
                args.push(format!("{host}:{container}:ro"));
            } else {
                args.push(format!("{host}:{container}"));
            }
        }

        for (name, container) in volumes {
            args.push("-v".to_string());
            args.push(format!("{name}:{container}"));
        }

        for (key, val) in env_vars {
            args.push("-e".to_string());
            args.push(format!("{key}={val}"));
        }

        if !working_dir.is_empty() {
            args.push("-w".to_string());
            args.push(working_dir.to_string());
        }

        args.push(image.to_string());
        args.extend_from_slice(command);

        let mut cmd = Command::new("docker");
        cmd.args(args);
        cmd
    }

    /// List all Docker volumes whose names start with the given prefix.
    /// Returns `(name, size_bytes)` pairs. Size is best-effort from `docker system df -v`.
    pub async fn list_cache_volumes(prefix: &str) -> Vec<(String, Option<u64>)> {
        let Ok(out) = tokio::process::Command::new("docker")
            .args(["volume", "ls", "--filter", &format!("name={prefix}"), "--format", "{{.Name}}"])
            .output()
            .await
        else {
            return vec![];
        };
        let names: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();

        let size_map = Self::volume_sizes().await;

        names
            .into_iter()
            .map(|n| {
                let sz = size_map.get(&n).copied();
                (n, sz)
            })
            .collect()
    }

    async fn volume_sizes() -> std::collections::HashMap<String, u64> {
        let Ok(out) = tokio::process::Command::new("docker")
            .args(["system", "df", "-v", "--format", "{{json .}}"])
            .output()
            .await
        else {
            return Default::default();
        };

        // `docker system df -v --format '{{json .}}'` emits one JSON object per entity.
        // Volume entries have {"Name":"...","Size":"1.2GB",...}.
        let mut map = std::collections::HashMap::new();
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            let Some(name) = v.get("Name").and_then(|n| n.as_str()) else {
                continue;
            };
            let Some(size_str) = v.get("Size").and_then(|s| s.as_str()) else {
                continue;
            };
            if let Some(bytes) = parse_docker_size(size_str) {
                map.insert(name.to_string(), bytes);
            }
        }
        map
    }

    /// Remove a named Docker volume. Returns true if successful.
    pub async fn remove_volume(name: &str) -> bool {
        tokio::process::Command::new("docker")
            .args(["volume", "rm", name])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

fn parse_docker_size(s: &str) -> Option<u64> {
    let s = s.trim();
    let (num, unit) = if let Some(n) = s.strip_suffix("GB") {
        (n.trim().parse::<f64>().ok()?, 1_000_000_000u64)
    } else if let Some(n) = s.strip_suffix("MB") {
        (n.trim().parse::<f64>().ok()?, 1_000_000u64)
    } else if let Some(n) = s.strip_suffix("kB") {
        (n.trim().parse::<f64>().ok()?, 1_000u64)
    } else if let Some(n) = s.strip_suffix("B") {
        (n.trim().parse::<f64>().ok()?, 1u64)
    } else {
        return None;
    };
    Some((num * unit as f64) as u64)
}
