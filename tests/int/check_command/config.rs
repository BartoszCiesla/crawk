use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// Explicit single-group config via -c is satisfied (exit 0).
#[test]
fn should_accept_explicit_clean_config() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_clean.toml")
    );
}

// An explicit config path that does not exist is an operational error (exit 2).
#[test]
fn should_error_on_missing_explicit_config() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/does_not_exist.toml")
    );
}
