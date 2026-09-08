use crate::common::{backtrace_filters, crate_root_filters, crawk_check, crawk_cycles};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// A rule naming a non-existent module → UnknownRuleModule (exit 2).
#[test]
fn should_error_on_unknown_module() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_bad_module.toml")
    );
}

// A deny rule naming a non-existent module → UnknownRuleModule (exit 2).
#[test]
fn should_error_on_unknown_module_in_deny() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_deny_bad_module.toml")
    );
}

// An allowlist entry naming a non-existent module → UnknownRuleModule (exit 2).
// Entries hold exact module names, so the typo cannot be a "pattern that matches
// nothing" — it is always a mistake.
#[test]
fn should_error_on_unknown_module_in_allow_cycle() {
    with_settings!({ filters => backtrace_filters() }, {
        assert_cmd_snapshot!(
            crawk_cycles()
                .arg("check")
                .arg("-c")
                .arg("fixtures/cycles/rules_allow_cycle_unknown.toml")
        );
    });
}

// A one-module allowlist entry can never match a cycle (an SCC has at least two
// modules), so it is rejected rather than silently ignored (exit 2).
#[test]
fn should_error_on_single_module_allow_cycle() {
    with_settings!({ filters => backtrace_filters() }, {
        assert_cmd_snapshot!(
            crawk_cycles()
                .arg("check")
                .arg("-c")
                .arg("fixtures/cycles/rules_allow_cycle_single.toml")
        );
    });
}

// The same loop listed twice (order does not matter — entries are sets) → exit 2.
#[test]
fn should_error_on_duplicate_allow_cycle() {
    with_settings!({ filters => backtrace_filters() }, {
        assert_cmd_snapshot!(
            crawk_cycles()
                .arg("check")
                .arg("-c")
                .arg("fixtures/cycles/rules_allow_cycle_dup.toml")
        );
    });
}

// `--init` refuses to clobber an existing config (the fixture has .crawk.toml),
// exiting 2. The absolute crate root is filtered out.
#[test]
fn init_refuses_when_config_exists() {
    with_settings!({
        filters => crate_root_filters(),
    }, {
        assert_cmd_snapshot!(crawk_check().arg("--init"));
    });
}
