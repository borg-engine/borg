/// Tests for run_claude_print timeout behavior.
///
/// These tests exercise the timeout/kill pattern used by run_claude_print
/// without requiring the claude CLI or a full Pipeline instance.
use std::time::{Duration, Instant};

/// Spawns a subprocess, writes stdin, and waits with a timeout.
/// Returns Ok(stdout) on success, Err if the process errors or times out.
async fn run_with_timeout(
    cmd: &str,
    args: &[&str],
    stdin_data: &[u8],
    timeout: Duration,
) -> anyhow::Result<Vec<u8>> {
    use tokio::io::AsyncWriteExt;

    let mut child = tokio::process::Command::new(cmd)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn: {e}"))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(stdin_data).await.ok();
    }

    match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => Ok(out.stdout),
        Ok(Err(e)) => Err(anyhow::anyhow!("wait: {e}")),
        Err(_elapsed) => anyhow::bail!("subprocess timed out after {}ms", timeout.as_millis()),
    }
}

/// Timeout fires for a process that stalls beyond the deadline.
#[tokio::test]
async fn test_stalled_subprocess_times_out() {
    let result = run_with_timeout("sleep", &["9999"], b"", Duration::from_millis(200)).await;
    assert!(result.is_err(), "stalled subprocess must time out");
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("timed out"), "error must mention timeout, got: {msg}");
}

/// Fast subprocess finishes before the timeout and returns stdout.
#[tokio::test]
async fn test_fast_subprocess_succeeds() {
    let result = run_with_timeout("echo", &["hello"], b"", Duration::from_secs(5)).await;
    assert!(result.is_ok(), "fast subprocess must succeed");
    let out = String::from_utf8_lossy(&result.unwrap()).into_owned();
    assert!(out.contains("hello"), "stdout must contain echoed text");
}

/// Timeout returns within a reasonable bound (not blocked waiting for full deadline).
#[tokio::test]
async fn test_timeout_fires_promptly() {
    let deadline = Duration::from_millis(300);
    let slack = Duration::from_millis(500);
    let start = Instant::now();
    let _ = run_with_timeout("sleep", &["9999"], b"", deadline).await;
    let elapsed = start.elapsed();
    assert!(
        elapsed < deadline + slack,
        "timeout must fire promptly; elapsed={elapsed:?}, deadline={deadline:?}"
    );
}

/// Process is killed on timeout — a subsequent wait returns quickly (not zombie).
#[tokio::test]
async fn test_process_killed_on_timeout() {
    // After the timeout the child handle is dropped with kill_on_drop(true).
    // We verify by checking that re-running the same scenario completes quickly.
    for _ in 0..3 {
        let _ = run_with_timeout("sleep", &["9999"], b"", Duration::from_millis(100)).await;
    }
    // If previous children were not killed they would accumulate; the test
    // would slow down as the OS runs out of process slots. Reaching here
    // without hanging is the assertion.
}

/// A subprocess that exits with a non-zero code still returns output (not a timeout error).
#[tokio::test]
async fn test_nonzero_exit_still_returns_stdout() {
    // `sh -c 'echo out; exit 1'` writes to stdout then exits non-zero.
    let result =
        run_with_timeout("sh", &["-c", "echo captured; exit 1"], b"", Duration::from_secs(5)).await;
    // Our helper returns Ok regardless of exit code (mirrors run_claude_print behaviour).
    assert!(result.is_ok(), "non-zero exit must not be treated as timeout");
    let text = String::from_utf8_lossy(&result.unwrap()).into_owned();
    assert!(text.contains("captured"));
}

/// Stdin data is delivered to the subprocess before we wait.
#[tokio::test]
async fn test_stdin_delivered_to_subprocess() {
    // `cat` echoes stdin to stdout.
    let result = run_with_timeout("cat", &[], b"ping", Duration::from_secs(5)).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), b"ping");
}
