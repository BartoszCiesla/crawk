use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Include-tests flag for modules with #[cfg(test)] blocks
// ============================================================================

#[test_matrix(
    ["lib", "tests", "inline_modules", "reexports", "glob_patterns",
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

// ============================================================================
// Shallow `-t` collection of an *external* test module
// ============================================================================

/// A file-based `#[cfg(test)] mod tests;` (no body) declared under a module
/// must be resolved to its own file (`bar/tests.rs`) in shallow (`-t`,
/// non-recursive) mode. Regressed when `collect_submodules_shallow` recorded
/// the external test module pointing at the *parent* file instead of resolving
/// it, so downstream inline descent found no body and produced a silently empty
/// analysis. The recursive form (`-t -r`) was already correct.
///
/// Kept as a flat `#[test] fn` (no wrapping `mod`) so it does not add a module
/// to crawk's own test-target tree, which the `list_command::test_targets`
/// snapshots enumerate.
#[test]
fn should_resolve_external_test_module_in_shallow_mode() {
    use crate::common::crawk;
    use std::fs;
    use tempfile::TempDir;

    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::create_dir(src.join("bar")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"shallowtest\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(src.join("lib.rs"), "pub mod bar;\npub mod dep;\n").unwrap();
    fs::write(src.join("dep.rs"), "pub struct D;\n").unwrap();
    // External test module: body lives in `src/bar/tests.rs`, not inline.
    fs::write(
        src.join("bar.rs"),
        "pub fn f() {}\n#[cfg(test)]\nmod tests;\n",
    )
    .unwrap();
    fs::write(
        src.join("bar").join("tests.rs"),
        "use crate::dep::D;\n#[test]\nfn t() {\n    let _ = D;\n}\n",
    )
    .unwrap();

    let output = crawk()
        .arg("-p")
        .arg(root.path())
        .arg("use")
        .arg("bar")
        .arg("-t")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("crate::dep::D"),
        "external `#[cfg(test)] mod tests;` was not resolved in shallow -t mode; stdout:\n{stdout}"
    );
}

/// An external `#[cfg(test)] mod tests;` declared **inside an inline module**
/// must resolve against the inline module's own directory. For a query on the
/// inline `foo::inner`, the base dir is extended by the inline ancestor
/// (`inner`), so `tests` resolves to `src/foo/inner/tests.rs` — where rustc
/// looks — rather than the containing file's directory. Exercises the
/// `inline_scope` fold in `collect_submodules_shallow` that the top-level case
/// leaves untouched.
///
/// Flat `#[test] fn` for the same reason as the sibling test above.
#[test]
fn should_resolve_external_test_module_inside_inline_module_in_shallow_mode() {
    use crate::common::crawk;
    use std::fs;
    use tempfile::TempDir;

    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::create_dir_all(src.join("foo").join("inner")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"inlineshallow\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(src.join("lib.rs"), "pub mod foo;\npub mod dep;\n").unwrap();
    fs::write(src.join("dep.rs"), "pub struct D;\n").unwrap();
    // `inner` is inline in foo.rs; its test module is external, body in
    // `src/foo/inner/tests.rs`.
    fs::write(
        src.join("foo.rs"),
        "pub mod inner {\n    pub fn f() {}\n    #[cfg(test)]\n    mod tests;\n}\n",
    )
    .unwrap();
    fs::write(
        src.join("foo").join("inner").join("tests.rs"),
        "use crate::dep::D;\n#[test]\nfn t() {\n    let _ = D;\n}\n",
    )
    .unwrap();

    let output = crawk()
        .arg("-p")
        .arg(root.path())
        .arg("use")
        .arg("foo::inner")
        .arg("-t")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("crate::dep::D"),
        "external test module inside an inline module was not resolved in shallow -t mode; stdout:\n{stdout}"
    );
}

/// An external `#[cfg(test)] mod tests;` with **no backing file** must not
/// crash or fabricate output: `resolve_module_parts` returns `None`, the module
/// is skipped (a `debug!` line), and analysis of the parent succeeds cleanly.
///
/// Flat `#[test] fn` for the same reason as the sibling tests above.
#[test]
fn should_skip_unresolved_external_test_module_in_shallow_mode() {
    use crate::common::crawk;
    use std::fs;
    use tempfile::TempDir;

    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"unresolvedshallow\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(src.join("lib.rs"), "pub mod bar;\n").unwrap();
    // Declares an external test module, but no `bar/tests.rs` exists to back it.
    fs::write(
        src.join("bar.rs"),
        "pub fn f() {}\n#[cfg(test)]\nmod tests;\n",
    )
    .unwrap();

    let output = crawk()
        .arg("-p")
        .arg(root.path())
        .arg("use")
        .arg("bar")
        .arg("-t")
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "unresolved external test module must not fail the run"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("tests"),
        "unresolved external test module must be skipped, not reported; stdout:\n{stdout}"
    );
}
