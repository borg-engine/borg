use borg_core::types::{PhaseConfig, PipelineMode};

fn make_mode(phase_names: &[&str]) -> PipelineMode {
    PipelineMode {
        name: "testborg".into(),
        label: "Test".into(),
        category: String::new(),
        phases: phase_names
            .iter()
            .map(|n| PhaseConfig {
                name: n.to_string(),
                label: n.to_string(),
                next: "done".into(),
                ..PhaseConfig::default()
            })
            .collect(),
        seed_modes: vec![],
        initial_status: "spec".into(),
        uses_git_worktrees: true,
        uses_docker: false,
        uses_test_cmd: false,
        integration: borg_core::types::IntegrationType::GitPr,
        default_max_attempts: 3,
    }
}

// ── get_phase ─────────────────────────────────────────────────────────────────

#[test]
fn test_get_phase_returns_correct_config() {
    let mode = make_mode(&["spec", "qa", "impl", "validate"]);
    let phase = mode.get_phase("impl").expect("impl phase must exist");
    assert_eq!(phase.name, "impl");
}

#[test]
fn test_get_phase_returns_none_for_unknown_name() {
    let mode = make_mode(&["spec", "impl"]);
    assert!(mode.get_phase("nonexistent").is_none());
}

#[test]
fn test_get_phase_returns_none_on_empty_phases() {
    let mode = make_mode(&[]);
    assert!(mode.get_phase("spec").is_none());
}

#[test]
fn test_get_phase_returns_first_match_when_names_unique() {
    let mode = make_mode(&["spec", "qa", "impl", "validate", "release"]);
    for name in &["spec", "qa", "impl", "validate", "release"] {
        let phase = mode.get_phase(name).unwrap_or_else(|| panic!("{name} must be found"));
        assert_eq!(&phase.name, name);
    }
}

// ── get_phase_index ───────────────────────────────────────────────────────────

#[test]
fn test_get_phase_index_correct_for_each_position() {
    let mode = make_mode(&["spec", "qa", "impl", "validate", "release"]);
    assert_eq!(mode.get_phase_index("spec"), Some(0));
    assert_eq!(mode.get_phase_index("qa"), Some(1));
    assert_eq!(mode.get_phase_index("impl"), Some(2));
    assert_eq!(mode.get_phase_index("validate"), Some(3));
    assert_eq!(mode.get_phase_index("release"), Some(4));
}

#[test]
fn test_get_phase_index_returns_none_for_unknown_name() {
    let mode = make_mode(&["spec", "impl"]);
    assert!(mode.get_phase_index("validate").is_none());
}

#[test]
fn test_get_phase_index_returns_none_on_empty_phases() {
    let mode = make_mode(&[]);
    assert!(mode.get_phase_index("spec").is_none());
}

#[test]
fn test_get_phase_index_ordering_matches_pipeline_sequence() {
    // Ordering must reflect the canonical spec→qa→impl→validate sequence.
    let mode = make_mode(&["spec", "qa", "impl", "validate"]);
    let spec_i = mode.get_phase_index("spec").unwrap();
    let qa_i = mode.get_phase_index("qa").unwrap();
    let impl_i = mode.get_phase_index("impl").unwrap();
    let validate_i = mode.get_phase_index("validate").unwrap();

    assert!(spec_i < qa_i, "spec must come before qa");
    assert!(qa_i < impl_i, "qa must come before impl");
    assert!(impl_i < validate_i, "impl must come before validate");
}

// ── is_terminal ───────────────────────────────────────────────────────────────

#[test]
fn test_is_terminal_done() {
    let mode = make_mode(&[]);
    assert!(mode.is_terminal("done"));
}

#[test]
fn test_is_terminal_merged() {
    let mode = make_mode(&[]);
    assert!(mode.is_terminal("merged"));
}

#[test]
fn test_is_terminal_failed() {
    let mode = make_mode(&[]);
    assert!(mode.is_terminal("failed"));
}

#[test]
fn test_is_terminal_false_for_active_phases() {
    let mode = make_mode(&[]);
    for status in &["spec", "qa", "impl", "validate", "release", "backlog", "blocked"] {
        assert!(!mode.is_terminal(status), "{status} must not be terminal");
    }
}

#[test]
fn test_is_terminal_false_for_empty_string() {
    let mode = make_mode(&[]);
    assert!(!mode.is_terminal(""));
}
