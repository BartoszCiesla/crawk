use crate::common::{crate_root_filters, crawk_check};
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
