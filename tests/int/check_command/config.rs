use crate::common::{crate_root_filters, crawk_check, crawk_modules};
use insta::with_settings;
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

// Auto-discovery against a crate with no config fails (exit 2) and points the
// user at `crawk check --init`. The absolute crate root is filtered out.
#[test]
fn missing_auto_config_hints_init() {
    with_settings!({
        filters => crate_root_filters(),
    }, {
        assert_cmd_snapshot!(crawk_modules().arg("check"));
    });
}
