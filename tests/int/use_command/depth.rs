use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Depth flag combinations for modules with deep hierarchies
// ============================================================================

#[test_matrix(
    ["lib", "nesting", "glob_patterns", "glob_showcase",
     "advanced_globs", "reexports"],
    [&["-d", "1"],
     &["-d", "2"],
     &["-d", "3"],
     &["-e", "-d", "1"],
     &["-e", "-d", "2"],
     &["-r", "-e", "-d", "1"],
     &["-r", "-e", "-d", "2"],
     &["-e", "-G"],
    ]
)]
fn should_modules_use_respect_depth(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_depth_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
