use crate::common::{backtrace_filters, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Custom path (#[path = "..."]) modules
// ============================================================================

#[test_case("custom_path"; "custom path module")]
#[test_case("aliased"; "aliased via path attribute")]
fn should_modules_use_handle_custom_path(module: &str) {
    let snapshot_name = format!("modules_{module}_custom_path");

    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(
            snapshot_name,
            crawk_modules().arg("use").arg(module).arg("-r").arg("-e")
        );
    });
}
