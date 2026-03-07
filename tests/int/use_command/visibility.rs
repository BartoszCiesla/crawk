use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Visibility sub-module tests
// ============================================================================
#[test_matrix(
    ["visibility::pub_mod", "visibility::pub_crate_mod",
     "visibility::pub_super_mod", "visibility::private_mod"],
    [&["-r"],
     &["-e"],
     &["-r", "-e"],
     &["-r", "-e", "--grouped"],
    ]
)]
fn should_modules_use_handle_visibility(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_vis_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("use").arg(module).args(flags)
    );
}
