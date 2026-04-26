use crate::common::{backtrace_filters, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Inline module sub-module tests
// ============================================================================

#[test_matrix(
    ["inline_modules::inner", "inline_modules::nested",
     "inline_modules::nested::deep"],
    [&["-e"],
     &["-r"],
     &["-r", "-e"],
    ]
)]
fn should_modules_use_handle_inline_modules(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_inline_{flags_part}", module.replace("::", "__"));

    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(
            snapshot_name,
            crawk_modules().arg("use").arg(module).args(flags)
        );
    });
}
