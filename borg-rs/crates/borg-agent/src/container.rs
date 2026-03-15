use std::process::Stdio;

use anyhow::{Context, Result};
use async_trait::async_trait;
use borg_core::{
    agent::AgentBackend,
    sandbox::Sandbox,
    traits::BackendCapabilities,
    types::{ContainerTestResult, PhaseConfig, PhaseContext, PhaseOutput, Task},
};
use serde_json::json;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tracing::{debug, info, warn};

const BORG_SIGNAL_MARKER: &str = "BORG_SIGNAL:";

/// Runs any CLI agent inside a Docker container. Provider-agnostic — the
/// container image determines what agent runs (Claude, Codex, custom, etc.).
pub struct ContainerBackend {
    pub docker_image: String,
    pub timeout_s: u64,
    pub container_memory_mb: u64,
    pub container_cpus: f64,
}

impl ContainerBackend {
    pub fn new(docker_image: impl Into<String>) -> Self {
        Self {
            docker_image: docker_image.into(),
            timeout_s: 0,
            container_memory_mb: 0,
            container_cpus: 0.0,
        }
    }

    pub fn with_timeout(mut self, timeout_s: u64) -> Self {
        self.timeout_s = timeout_s;
        self
    }

    pub fn with_resource_limits(mut self, memory_mb: u64, cpus: f64) -> Self {
        self.container_memory_mb = memory_mb;
        self.container_cpus = cpus;
        self
    }
}

fn container_host_ip(isolated: bool) -> &'static str {
    if isolated {
        "172.31.0.1"
    } else {
        "172.30.0.1"
    }
}

fn container_reachable_url(base_url: &str, host_ip: &str) -> String {
    if let Some(port) = base_url.strip_prefix("http://127.0.0.1:") {
        return format!("http://{host_ip}:{port}");
    }
    if let Some(port) = base_url.strip_prefix("http://localhost:") {
        return format!("http://{host_ip}:{port}");
    }
    base_url.to_string()
}

#[async_trait]
impl AgentBackend for ContainerBackend {
    async fn run_phase(
        &self,
        task: &Task,
        phase: &PhaseConfig,
        ctx: PhaseContext,
    ) -> Result<PhaseOutput> {
        let instruction = crate::instruction::build_instruction(task, phase, &ctx, None);
        let host_ip = container_host_ip(ctx.isolated);
        let reachable_borg_api_url = container_reachable_url(&ctx.borg_api_url, host_ip);

        let workspace_host = if !task.repo_path.is_empty()
            && std::path::Path::new(&task.repo_path).join(".git").exists()
        {
            task.repo_path.clone()
        } else {
            ctx.work_dir.clone()
        };

        let binds = [(workspace_host, "/workspace".to_string(), false),
            (ctx.session_dir.clone(), "/home/bun".to_string(), false)];
        let volumes_owned = [("rustup-cache".to_string(), "/home/bun/.rustup".to_string()),
            ("cargo-cache".to_string(), "/home/bun/.cargo".to_string())];

        let mut env_kv = vec![
            ("HOME".to_string(), "/home/bun".to_string()),
            ("RUSTUP_HOME".to_string(), "/home/bun/.rustup".to_string()),
            ("CARGO_HOME".to_string(), "/home/bun/.cargo".to_string()),
        ];
        if !ctx.oauth_token.is_empty() {
            env_kv.push((
                "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
                ctx.oauth_token.clone(),
            ));
        }
        if !ctx.github_token.is_empty() {
            env_kv.push(("GH_TOKEN".to_string(), ctx.github_token.clone()));
        }
        if !reachable_borg_api_url.is_empty() {
            env_kv.push(("API_BASE_URL".to_string(), reachable_borg_api_url.clone()));
        }
        if !ctx.borg_api_token.is_empty() {
            env_kv.push(("API_TOKEN".to_string(), ctx.borg_api_token.clone()));
        }
        env_kv.push(("BORG_HOST_IP".to_string(), host_ip.to_string()));

        let binds_ref: Vec<(&str, &str, bool)> = binds
            .iter()
            .map(|(h, c, r)| (h.as_str(), c.as_str(), *r))
            .collect();
        let volumes_ref: Vec<(&str, &str)> = volumes_owned
            .iter()
            .map(|(n, c)| (n.as_str(), c.as_str()))
            .collect();
        let env_ref: Vec<(&str, &str)> = env_kv
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();

        let mut docker_cmd = Sandbox::docker_command(
            &self.docker_image,
            &binds_ref,
            &volumes_ref,
            "",
            &[],
            &env_ref,
            self.container_memory_mb,
            self.container_cpus,
            ctx.agent_network.as_deref(),
        );

        docker_cmd
            .kill_on_drop(true)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::piped());

        info!(
            task_id = task.id,
            phase = %phase.name,
            image = %self.docker_image,
            "spawning container agent"
        );

        let stream_tx = ctx.stream_tx.clone();
        if let Some(tx) = &stream_tx {
            let evt =
                json!({"type": "status", "status": "Spawning agent (Container)..."}).to_string();
            let _ = tx.send(evt);
        }

        let compile_check_cmd = if phase.compile_check {
            borg_core::pipeline::derive_compile_check(&ctx.repo_config.test_cmd).unwrap_or_default()
        } else {
            String::new()
        };

        let mut child = docker_cmd.spawn().context("failed to spawn docker")?;

        if let Some(mut stdin) = child.stdin.take() {
            let input = json!({
                "prompt": instruction,
                "model": &ctx.model,
                "sessionId": task.session_id,
                "systemPrompt": phase.system_prompt,
                "allowedTools": phase.allowed_tools,
                "maxTurns": 200,
                "projectId": task.project_id,
                "testCmd": ctx.repo_config.test_cmd,
                "compileCheckCmd": compile_check_cmd,
                "lintCmd": ctx.repo_config.lint_cmd,
            });
            let payload = serde_json::to_vec(&input).unwrap_or_default();
            let _ = stdin.write_all(&payload).await;
            drop(stdin);
        }

        let stdout = child.stdout.take().context("no stdout")?;
        let stderr = child.stderr.take().context("no stderr")?;

        let timeout_s = self.timeout_s;
        let io_future = async move {
            let mut signal_json: Option<String> = None;
            let mut output_lines: Vec<String> = Vec::new();
            let mut container_test_results: Vec<ContainerTestResult> = Vec::new();

            let mut stdout_reader = BufReader::new(stdout).lines();
            let mut stderr_reader = BufReader::new(stderr).lines();
            let mut stdout_done = false;
            let mut stderr_done = false;

            while !stdout_done || !stderr_done {
                tokio::select! {
                    line = stdout_reader.next_line(), if !stdout_done => {
                        match line {
                            Ok(Some(l)) => {
                                if let Some(sig) = l.strip_prefix(BORG_SIGNAL_MARKER) {
                                    signal_json = Some(sig.to_string());
                                }
                                if let Some(tx) = &stream_tx {
                                    let _ = tx.send(l.clone());
                                }
                                if output_lines.len() < 50_000 && l.len() < 100_000 {
                                    output_lines.push(l);
                                }
                            }
                            Ok(None) => stdout_done = true,
                            Err(e) => { warn!("stdout error: {e}"); stdout_done = true; }
                        }
                    }
                    line = stderr_reader.next_line(), if !stderr_done => {
                        match line {
                            Ok(Some(l)) => {
                                if !l.is_empty() {
                                    let test_line = l.strip_prefix("---BORG_TEST_RESULT---").unwrap_or(&l);
                                    if let Ok(res) = serde_json::from_str::<ContainerTestResult>(test_line) {
                                        container_test_results.push(res);
                                    } else {
                                        debug!("container stderr: {l}");
                                    }
                                }
                            }
                            Ok(None) => stderr_done = true,
                            Err(_) => stderr_done = true,
                        }
                    }
                }
            }

            let exit_status = child.wait().await.ok();
            let success = exit_status.map(|s| s.success()).unwrap_or(false);
            (
                output_lines.join("\n"),
                signal_json,
                container_test_results,
                success,
            )
        };

        let (stdout_text, signal_json, container_test_results, success) = if timeout_s > 0 {
            match tokio::time::timeout(std::time::Duration::from_secs(timeout_s), io_future).await {
                Ok(res) => res,
                Err(_) => {
                    warn!("container timed out after {timeout_s}s");
                    (String::new(), None, Vec::new(), false)
                },
            }
        } else {
            io_future.await
        };

        Ok(PhaseOutput {
            output: stdout_text,
            new_session_id: None,
            raw_stream: String::new(),
            success,
            signal_json,
            ran_in_docker: true,
            container_test_results,
        })
    }

    fn name(&self) -> &str {
        "container"
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            supports_mcp: true,
            supports_sessions: true,
            supports_tools: true,
            supports_streaming: true,
            supports_sandbox: true,
            supported_models: vec![],
        }
    }
}
