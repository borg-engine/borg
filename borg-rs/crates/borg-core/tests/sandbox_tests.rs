use borg_core::sandbox::Sandbox;
use tempfile::TempDir;

// ── helpers ───────────────────────────────────────────────────────────────────

fn cmd(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

/// Position of the first occurrence of `flag` in `args`, or None.
fn pos(args: &[String], flag: &str) -> Option<usize> {
    args.iter().position(|a| a == flag)
}

fn has(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

// ── canonical argument order (no writable dirs) ───────────────────────────────

#[test]
fn bwrap_args_starts_with_ro_bind_root() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    assert_eq!(&args[..3], &["--ro-bind", "/", "/"]);
}

#[test]
fn bwrap_args_dev_follows_ro_bind() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    let ro_bind = pos(&args, "--ro-bind").unwrap();
    let dev = pos(&args, "--dev").unwrap();
    // --dev /dev immediately follows --ro-bind / /
    assert_eq!(dev, ro_bind + 3, "--dev must appear right after --ro-bind / /");
    assert_eq!(args[dev + 1], "/dev");
}

#[test]
fn bwrap_args_tmp_bind_precedes_namespace_flags() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    let tmp_bind = args.windows(3).position(|w| w == ["--bind", "/tmp", "/tmp"]).unwrap();
    let unshare_pid = pos(&args, "--unshare-pid").unwrap();
    assert!(tmp_bind < unshare_pid, "--bind /tmp /tmp must come before --unshare-pid");
}

#[test]
fn bwrap_args_proc_follows_die_with_parent() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    let dwp = pos(&args, "--die-with-parent").unwrap();
    let proc = pos(&args, "--proc").unwrap();
    assert_eq!(proc, dwp + 1, "--proc must immediately follow --die-with-parent");
    assert_eq!(args[proc + 1], "/proc");
}

#[test]
fn bwrap_args_chdir_precedes_separator() {
    let args = Sandbox::bwrap_args(&[], "/my/workdir", &cmd(&["sh"]), false);
    let chdir = pos(&args, "--chdir").unwrap();
    let sep = pos(&args, "--").unwrap();
    assert!(chdir < sep, "--chdir must precede --");
    assert_eq!(args[chdir + 1], "/my/workdir");
}

#[test]
fn bwrap_args_command_follows_separator() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh", "-c", "echo hi"]), false);
    let sep = pos(&args, "--").unwrap();
    assert_eq!(&args[sep + 1..], &["sh", "-c", "echo hi"]);
}

// ── writable dirs ─────────────────────────────────────────────────────────────

#[test]
fn bwrap_args_existing_writable_dir_appears_after_dev() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap();

    let args = Sandbox::bwrap_args(&[dir], "/work", &cmd(&["sh"]), false);
    let dev = pos(&args, "--dev").unwrap();

    let bind_pos = args
        .windows(3)
        .position(|w| w == ["--bind", dir, dir])
        .expect("writable dir bind must be present");

    assert!(bind_pos > dev, "writable --bind must come after --dev");
}

#[test]
fn bwrap_args_existing_writable_dir_precedes_tmp_bind() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().to_str().unwrap();

    let args = Sandbox::bwrap_args(&[dir], "/work", &cmd(&["sh"]), false);

    let bind_pos = args
        .windows(3)
        .position(|w| w == ["--bind", dir, dir])
        .unwrap();
    let tmp_pos = args
        .windows(3)
        .position(|w| w == ["--bind", "/tmp", "/tmp"])
        .unwrap();

    assert!(bind_pos < tmp_pos, "writable --bind must precede --bind /tmp /tmp");
}

#[test]
fn bwrap_args_multiple_writable_dirs_in_order() {
    let a = TempDir::new().unwrap();
    let b = TempDir::new().unwrap();
    let da = a.path().to_str().unwrap();
    let db = b.path().to_str().unwrap();

    let args = Sandbox::bwrap_args(&[da, db], "/work", &cmd(&["sh"]), false);

    let pos_a = args.windows(3).position(|w| w == ["--bind", da, da]).unwrap();
    let pos_b = args.windows(3).position(|w| w == ["--bind", db, db]).unwrap();
    assert!(pos_a < pos_b, "first writable dir must appear before second");
}

#[test]
fn bwrap_args_nonexistent_dir_is_skipped() {
    let args = Sandbox::bwrap_args(
        &["/this/path/cannot/possibly/exist/xyz123"],
        "/work",
        &cmd(&["sh"]),
        false,
    );
    assert!(
        !has(&args, "/this/path/cannot/possibly/exist/xyz123"),
        "non-existent dir must not appear in args"
    );
}

#[test]
fn bwrap_args_nonexistent_dir_does_not_affect_other_mounts() {
    let args = Sandbox::bwrap_args(
        &["/no/such/dir"],
        "/work",
        &cmd(&["sh"]),
        false,
    );
    // Fixed mounts must still be present
    assert!(has(&args, "--ro-bind"), "ro-bind root must still be present");
    assert!(has(&args, "--dev"), "--dev must still be present");
    assert!(
        args.windows(3).any(|w| w == ["--bind", "/tmp", "/tmp"]),
        "--bind /tmp /tmp must still be present"
    );
}

// ── network namespace flag ────────────────────────────────────────────────────

#[test]
fn bwrap_args_no_network_includes_unshare_net() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    assert!(has(&args, "--unshare-net"), "--unshare-net must be present when allow_network=false");
}

#[test]
fn bwrap_args_allow_network_omits_unshare_net() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), true);
    assert!(!has(&args, "--unshare-net"), "--unshare-net must be absent when allow_network=true");
}

#[test]
fn bwrap_args_unshare_net_after_unshare_pid() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    let pid = pos(&args, "--unshare-pid").unwrap();
    let net = pos(&args, "--unshare-net").unwrap();
    assert!(net > pid, "--unshare-net must appear after --unshare-pid");
}

#[test]
fn bwrap_args_unshare_net_before_separator() {
    let args = Sandbox::bwrap_args(&[], "/work", &cmd(&["sh"]), false);
    let net = pos(&args, "--unshare-net").unwrap();
    let sep = pos(&args, "--").unwrap();
    assert!(net < sep, "--unshare-net must appear before --");
}
