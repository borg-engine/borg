use borg_core::sandbox::{Sandbox, SandboxMode};

// --- SandboxMode::from_str_or_auto ---

#[test]
fn from_str_bwrap() {
    assert_eq!(SandboxMode::from_str_or_auto("bwrap"), Some(SandboxMode::Bwrap));
}

#[test]
fn from_str_bwrap_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("BWRAP"), Some(SandboxMode::Bwrap));
}

#[test]
fn from_str_bwrap_mixed_case() {
    assert_eq!(SandboxMode::from_str_or_auto("Bwrap"), Some(SandboxMode::Bwrap));
}

#[test]
fn from_str_docker() {
    assert_eq!(SandboxMode::from_str_or_auto("docker"), Some(SandboxMode::Docker));
}

#[test]
fn from_str_docker_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("DOCKER"), Some(SandboxMode::Docker));
}

#[test]
fn from_str_none_maps_to_direct() {
    assert_eq!(SandboxMode::from_str_or_auto("none"), Some(SandboxMode::Direct));
}

#[test]
fn from_str_none_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("NONE"), Some(SandboxMode::Direct));
}

#[test]
fn from_str_direct() {
    assert_eq!(SandboxMode::from_str_or_auto("direct"), Some(SandboxMode::Direct));
}

#[test]
fn from_str_direct_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("DIRECT"), Some(SandboxMode::Direct));
}

#[test]
fn from_str_auto_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("auto"), None);
}

#[test]
fn from_str_unrecognised_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("podman"), None);
    assert_eq!(SandboxMode::from_str_or_auto(""), None);
    assert_eq!(SandboxMode::from_str_or_auto("unknown"), None);
}

// --- Sandbox::branch_hash ---

#[test]
fn branch_hash_is_8_chars() {
    let h = Sandbox::branch_hash("main");
    assert_eq!(h.len(), 8, "hash must be 8 characters, got {h:?}");
}

#[test]
fn branch_hash_is_hex() {
    let h = Sandbox::branch_hash("feature/foo");
    assert!(
        h.chars().all(|c| c.is_ascii_hexdigit()),
        "hash must be hex digits, got {h:?}"
    );
}

#[test]
fn branch_hash_is_deterministic() {
    assert_eq!(Sandbox::branch_hash("main"), Sandbox::branch_hash("main"));
    assert_eq!(
        Sandbox::branch_hash("feature/my-branch"),
        Sandbox::branch_hash("feature/my-branch")
    );
}

#[test]
fn branch_hash_differs_for_different_branches() {
    assert_ne!(Sandbox::branch_hash("main"), Sandbox::branch_hash("develop"));
    assert_ne!(
        Sandbox::branch_hash("feature/foo"),
        Sandbox::branch_hash("feature/bar")
    );
}

// --- Sandbox::branch_volume_name ---

#[test]
fn branch_volume_name_format() {
    let name = Sandbox::branch_volume_name("myrepo", "main", "target");
    let hash = Sandbox::branch_hash("main");
    assert_eq!(name, format!("borg-cache-myrepo-{hash}-target"));
}

#[test]
fn branch_volume_name_starts_with_prefix() {
    let name = Sandbox::branch_volume_name("zrchain", "feature/test", "cargo");
    assert!(name.starts_with("borg-cache-zrchain-"), "got: {name}");
    assert!(name.ends_with("-cargo"), "got: {name}");
}

#[test]
fn branch_volume_name_different_branches_differ() {
    let a = Sandbox::branch_volume_name("repo", "main", "target");
    let b = Sandbox::branch_volume_name("repo", "develop", "target");
    assert_ne!(a, b);
}

#[test]
fn branch_volume_name_different_types_differ() {
    let a = Sandbox::branch_volume_name("repo", "main", "target");
    let b = Sandbox::branch_volume_name("repo", "main", "cargo");
    assert_ne!(a, b);
}
