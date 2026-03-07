use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Include-tests flag for modules with #[cfg(test)] blocks
// ============================================================================

#[test_matrix(
    ["lib", "inline_modules", "reexports", "glob_patterns",
     "advanced_globs", "glob_showcase"],
    [&["-t"],
     &["-t", "-e"],
     &["-r", "-t"],
     &["-r", "-t", "-e"],
     &["-r", "-t", "-e", "--resolve-globs"],
    ]
)]
fn should_modules_use_include_tests(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_tests_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
