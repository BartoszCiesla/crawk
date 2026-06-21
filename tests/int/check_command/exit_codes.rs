use crate::common::crawk_check;

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
fn overlapping_groups_is_two() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/rules_overlapping_groups.toml"]),
        Some(2)
    );
}

#[test]
fn missing_config_is_two() {
    assert_eq!(
        exit_code(&["-c", "fixtures/check/does_not_exist.toml"]),
        Some(2)
    );
}
