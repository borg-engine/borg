use borg_core::sandbox::SandboxMode;

#[test]
fn test_bwrap_lowercase() {
    assert_eq!(SandboxMode::from_str_or_auto("bwrap"), Some(SandboxMode::Bwrap));
}

#[test]
fn test_docker_lowercase() {
    assert_eq!(SandboxMode::from_str_or_auto("docker"), Some(SandboxMode::Docker));
}

#[test]
fn test_direct_lowercase() {
    assert_eq!(SandboxMode::from_str_or_auto("direct"), Some(SandboxMode::Direct));
}

#[test]
fn test_none_maps_to_direct() {
    assert_eq!(SandboxMode::from_str_or_auto("none"), Some(SandboxMode::Direct));
}

#[test]
fn test_auto_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("auto"), None);
}

#[test]
fn test_empty_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto(""), None);
}

#[test]
fn test_unknown_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("podman"), None);
    assert_eq!(SandboxMode::from_str_or_auto("sandbox"), None);
    assert_eq!(SandboxMode::from_str_or_auto("disabled"), None);
}

#[test]
fn test_bwrap_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("BWRAP"), Some(SandboxMode::Bwrap));
}

#[test]
fn test_docker_mixed_case() {
    assert_eq!(SandboxMode::from_str_or_auto("Docker"), Some(SandboxMode::Docker));
    assert_eq!(SandboxMode::from_str_or_auto("DOCKER"), Some(SandboxMode::Docker));
}

#[test]
fn test_direct_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("DIRECT"), Some(SandboxMode::Direct));
}

#[test]
fn test_none_uppercase() {
    assert_eq!(SandboxMode::from_str_or_auto("NONE"), Some(SandboxMode::Direct));
}

#[test]
fn test_auto_uppercase_returns_none() {
    assert_eq!(SandboxMode::from_str_or_auto("AUTO"), None);
}
