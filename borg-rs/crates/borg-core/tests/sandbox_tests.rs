use borg_core::sandbox::SandboxMode;

#[test]
fn bwrap_lowercase() {
    assert_eq!(SandboxMode::from_str_or_auto("bwrap"), Some(SandboxMode::Bwrap));
}

#[test]
fn bwrap_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("BWRAP"), Some(SandboxMode::Bwrap));
}

#[test]
fn bwrap_mixed_case() {
    assert_eq!(SandboxMode::from_str_or_auto("Bwrap"), Some(SandboxMode::Bwrap));
}

#[test]
fn docker_lowercase() {
    assert_eq!(SandboxMode::from_str_or_auto("docker"), Some(SandboxMode::Docker));
}

#[test]
fn docker_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("DOCKER"), Some(SandboxMode::Docker));
}

#[test]
fn none_returns_direct() {
    assert_eq!(SandboxMode::from_str_or_auto("none"), Some(SandboxMode::Direct));
}

#[test]
fn none_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("NONE"), Some(SandboxMode::Direct));
}

#[test]
fn direct_returns_direct() {
    assert_eq!(SandboxMode::from_str_or_auto("direct"), Some(SandboxMode::Direct));
}

#[test]
fn direct_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("DIRECT"), Some(SandboxMode::Direct));
}

#[test]
fn auto_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("auto"), None);
}

#[test]
fn unknown_string_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("podman"), None);
}

#[test]
fn empty_string_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto(""), None);
}

#[test]
fn blank_string_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("   "), None);
}
