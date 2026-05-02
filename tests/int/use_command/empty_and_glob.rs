use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Empty module and glob edge cases
// ============================================================================

// Test empty_module with various flags — it has no crate-internal deps.
// Exercises "No internal crate use statements found" path.
#[test_matrix(
    ["empty_module"],
    [&["-e"],
     &["-r"],
     &["-r", "-e"],
     &["-r", "-e", "--format", "grouped"],
    ]
)]
fn should_modules_use_handle_empty_module(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_empty_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}

// Test fallback: glob referencing a module that does not exist in the crate.
// With --resolve-globs, resolve_module_path_to_file returns Err → original glob
// must be preserved in output (not dropped, not panic).
#[test]
fn should_preserve_glob_when_module_unresolvable() {
    assert_cmd_snapshot!(
        crawk()
            .arg("-p")
            .arg("fixtures/glob_fallback")
            .arg("use")
            .arg("importer")
            .arg("--resolve-globs")
    );
}

// Test uses_empty_glob with --resolve-globs — glob on no_pub_items resolves to nothing.
// Exercises empty public items return path.
#[test_matrix(
    ["uses_empty_glob"],
    [&["-e"],
     &["-e", "--resolve-globs"],
     &["-r", "-e", "--resolve-globs"],
     &["-r", "-e", "--format", "grouped", "--resolve-globs"],
    ]
)]
fn should_modules_use_handle_empty_glob(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_glob_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
