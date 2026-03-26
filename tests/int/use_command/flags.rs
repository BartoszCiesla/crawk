use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Flag matrix for representative fixture modules
// ============================================================================

#[test_matrix(
    ["lib", "main", "app", "inline_modules", "reexports", "visibility",
     "nesting", "glob_patterns", "advanced_globs", "glob_showcase",
     "dir_module", "mixed"],
    [&["-e"],
     &["--resolve-globs"],
     &["-e", "--resolve-globs"],
     &["-r"],
     &["-r", "-e"],
     &["-r", "--resolve-globs"],
     &["-r", "-e", "--resolve-globs"],
     &["-r", "-e", "--format", "grouped"],
     &["-r", "-e", "--format", "grouped", "--resolve-globs"],
    ]
)]
fn should_modules_use_provide_output_for_flags(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_flags_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
