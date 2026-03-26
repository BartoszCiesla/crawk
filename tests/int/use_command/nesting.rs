use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Deep nesting sub-module traversal
// ============================================================================

#[test_matrix(
    ["nesting::level1", "nesting::level1::level2",
     "nesting::level1::level2::level3"],
    [&["-r"],
     &["-r", "-e"],
     &["-r", "-e", "-d", "1"],
     &["-r", "-e", "-d", "2"],
     &["-r", "-e", "--format", "grouped"],
    ]
)]
fn should_modules_use_handle_deep_nesting(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!(
        "modules_{}_nesting_{flags_part}",
        module.replace("::", "__")
    );

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
