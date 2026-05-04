use crate::common::{backtrace_filters, crawk};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Error scenarios — invalid flags and bad crate paths
// ============================================================================

#[test]
fn should_error_for_invalid_depth_zero() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("-d").arg("0"));
}

#[test]
fn should_error_for_invalid_depth_non_numeric() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("-d").arg("abc"));
}

#[test]
fn should_error_for_invalid_crate_path() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk().arg("-p").arg("/nonexistent/path").arg("deps"));
    });
}
