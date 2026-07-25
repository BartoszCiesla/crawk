use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Binary targets (main.rs and app.rs) — depth and include-tests
// ============================================================================

#[test_matrix(
    ["main", "app"],
    [&["-d", "1"],
     &["-d", "2"],
     &["-e", "-d", "1"],
     &["-e", "-d", "2"],
     &["-t"],
     &["-t", "-e"],
     &["-t", "-e", "--resolve-globs"],
     &["-e", "--format", "grouped"],
     &["-e", "--format", "grouped", "--resolve-globs"],
    ]
)]
fn should_modules_use_handle_binaries(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_bin_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}

// ============================================================================
// Regression: non-standard-path bin target root
// ============================================================================

/// `app.rs` (bin target `modules-cli`) is not named `main.rs`/`lib.rs`. An
/// inline submodule scoped to it must not leak the file's top-level `use`
/// statements — it has none of its own, so output must be empty.
#[test]
fn should_not_leak_top_level_uses_into_inline_module_of_custom_path_bin_target() {
    assert_cmd_snapshot!(crawk_modules().arg("use").arg("app::app_only"));
}
