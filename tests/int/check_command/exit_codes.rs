use super::temp_bare_crate;
use crate::common::{crawk, crawk_check};

// The CI contract: clean = 0, violations = 1, operational error = 2. Asserted
// explicitly (not via snapshot) because these codes are the command's promise.
fn exit_code(args: &[&str]) -> Option<i32> {
    crawk_check().args(args).output().ok()?.status.code()
}

#[test]
fn clean_is_zero() {
    assert_eq!(exit_code(&[]), Some(0));
}

#[test]
fn violation_is_one() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_layers_violated.toml"]),
        Some(1)
    );
}

#[test]
fn unknown_module_is_two() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_bad_module.toml"]),
        Some(2)
    );
}

#[test]
fn denied_edge_is_one() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_deny_violated.toml"]),
        Some(1)
    );
}

#[test]
fn satisfied_deny_is_zero() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_deny_clean.toml"]),
        Some(0)
    );
}

#[test]
fn unknown_module_in_deny_is_two() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_deny_bad_module.toml"]),
        Some(2)
    );
}

// Overlapping groups are valid config: the shared module is checked in each
// group, so this fixture lints (exit 1) rather than erroring (exit 2).
#[test]
fn overlapping_groups_violation_is_one() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_overlapping_groups.toml"]),
        Some(1)
    );
}

// Overlap alone, with every shared edge downward, is clean (exit 0).
#[test]
fn overlapping_groups_clean_is_zero() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_overlapping_clean.toml"]),
        Some(0)
    );
}

#[test]
fn missing_config_is_two() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/does_not_exist.toml"]),
        Some(2)
    );
}

// `--init` against a crate with no existing config succeeds (exit 0), same as
// any other clean run.
#[test]
fn init_success_is_zero() {
    let dir = temp_bare_crate();
    let root = dir.path().to_str().expect("utf8 path");
    let code = crawk()
        .arg("-p")
        .arg(root)
        .arg("check")
        .arg("--init")
        .output()
        .ok()
        .and_then(|o| o.status.code());
    assert_eq!(code, Some(0));
}
