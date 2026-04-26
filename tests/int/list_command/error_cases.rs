use crate::common::{backtrace_filters, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

#[test]
fn should_error_for_nonexistent_module() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk_modules().arg("list").arg("nonexistent_module"));
    });
}

#[test]
fn should_error_for_invalid_depth_zero() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-d").arg("0"));
}

#[test]
fn should_error_for_invalid_depth_non_numeric() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-d").arg("abc"));
}
