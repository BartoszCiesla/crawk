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
     &["-e", "--grouped"],
     &["-e", "--grouped", "--resolve-globs"],
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
