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

// ============================================================================
// Package-name-prefixed globs must honor pub(super) visibility
// ============================================================================
mod package_name_glob {
    use crate::common::crawk;
    use std::fs;
    use tempfile::TempDir;

    /// A glob written with the crate-name prefix (`mycrate::foo::*`) must resolve
    /// `pub(super)` items exactly like the `crate::foo::*` form. `foo::helper` is
    /// top-level `pub(super)`, so it is crate-wide visible and the `caller` module
    /// glob-importing it must see it. Regressed when `resolve_glob` passed the
    /// unstripped `mycrate::foo` as the visibility target, making its parent
    /// `mycrate` instead of the crate root and hiding `helper` from every caller.
    #[test]
    fn should_resolve_pub_super_through_package_name_glob() {
        let root = TempDir::new().unwrap();
        let src = root.path().join("src");
        fs::create_dir(&src).unwrap();
        fs::write(
            root.path().join("Cargo.toml"),
            "[package]\nname = \"pkgglob\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .unwrap();
        fs::write(src.join("lib.rs"), "pub mod foo;\npub mod caller;\n").unwrap();
        fs::write(
            src.join("foo.rs"),
            "pub(super) fn helper() {}\npub fn public_fn() {}\n",
        )
        .unwrap();
        // Package-name-prefixed glob (`pkgglob::foo::*`), not `crate::foo::*`.
        fs::write(src.join("caller.rs"), "use pkgglob::foo::*;\n").unwrap();

        let output = crawk()
            .arg("-p")
            .arg(root.path())
            .arg("use")
            .arg("caller")
            .arg("-G")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            stdout.contains("helper"),
            "pub(super) helper wrongly hidden through package-name glob; stdout:\n{stdout}"
        );
        assert!(
            stdout.contains("public_fn"),
            "pub public_fn missing from resolved glob; stdout:\n{stdout}"
        );
    }
}
