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
     &["-r", "-e", "--format", "grouped"],
     &["-e", "-G"],
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

// ============================================================================
// inner: glob-import from parent, exercises pub(in path) resolution
// ============================================================================
mod inner_glob {
    use crate::common::crawk_modules;
    use insta_cmd::assert_cmd_snapshot;
    use test_case::test_matrix;

    #[test_matrix(
        ["visibility::inner"],
        [&["-r"],
         &["--resolve-globs"],
         &["-r", "--resolve-globs"],
         &["-r", "-e", "--resolve-globs"],
         &["-r", "-e", "--format", "grouped", "--resolve-globs"],
        ]
    )]
    fn should_resolve_glob_from_visibility_inner(module: &str, flags: &[&str]) {
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
}

// ============================================================================
// Restricted visibility (pub(in path)) tests
// ============================================================================
mod restricted {
    use crate::common::crawk_modules;
    use insta_cmd::assert_cmd_snapshot;
    use test_case::test_matrix;

    #[test_matrix(
        ["visibility::restricted_mod"],
        [&["-r"],
         &["-e"],
         &["-r", "-e"],
         &["-r", "-e", "--format", "grouped"],
         &["-e", "-G"],
        ]
    )]
    fn should_modules_use_handle_restricted_visibility(module: &str, flags: &[&str]) {
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
}
