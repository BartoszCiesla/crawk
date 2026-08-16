use crate::common::{backtrace_filters, crawk};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Parse error — module context in error message
// ============================================================================

#[test]
fn should_fail_with_parse_error_including_module_context() {
    let mut filters = backtrace_filters();
    filters.push((env!("CARGO_MANIFEST_DIR"), "[MANIFEST_DIR]"));
    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p")
            .arg("fixtures/broken_syntax")
            .arg("use")
            .arg("broken"));
    });
}

// ============================================================================
// Parse error in a submodule — surfaced, not masked as "module not found"
// ============================================================================

/// A non-recursive query still builds a *recursive* module tree internally to
/// populate `children_map` (so bare child paths resolve). When a submodule
/// fails to parse, that discovery errors — and the integration-test-target
/// fallback must not swallow it and report the generic "Module not found" for
/// the healthy queried module instead.
///
/// `crawk use good -r` already reported the parse error correctly; without
/// `-r` it did not, so the two forms contradicted each other on the same crate.
#[test]
fn should_report_submodule_parse_error_in_non_recursive_mode() {
    use std::fs;
    use tempfile::TempDir;

    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::create_dir(src.join("good")).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"test-submodule-parse-error\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    fs::write(src.join("lib.rs"), "pub mod good;\npub mod dep;\n").unwrap();
    fs::write(src.join("dep.rs"), "pub struct D;\n").unwrap();
    fs::write(
        src.join("good.rs"),
        "pub mod broken;\npub use crate::dep::D;\n",
    )
    .unwrap();
    fs::write(src.join("good/broken.rs"), "this is not valid rust !!!\n").unwrap();

    let root_str = root.path().to_str().unwrap_or("");
    let mut filters: Vec<(&str, &str)> = vec![(root_str, "[ROOT]")];
    filters.extend(backtrace_filters());
    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p").arg(root.path())
            .arg("use")
            .arg("good"));
    });
}

// ============================================================================
// Invalid depth values
// ============================================================================

#[test_case("0"; "zero depth")]
#[test_case("abc"; "non numeric depth")]
fn should_fail_with_invalid_depth(depth: &str) {
    let snapshot_name = format!(
        "invalid_depth_{}",
        depth
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
    );

    assert_cmd_snapshot!(
        snapshot_name,
        crawk().arg("use").arg("lib").arg("-d").arg(depth)
    );
}

// ============================================================================
// Path traversal defense — CLI validation layer (layer 1)
// ============================================================================

#[test_case("foo::..::lib"; "dotdot segment")]
#[test_case("foo::/etc/passwd"; "absolute path segment")]
fn should_reject_path_traversal_at_cli(module_path: &str) {
    let snapshot_name = format!(
        "path_traversal_{}",
        module_path
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
    );

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module_path));
}

// ============================================================================
// Path traversal defense — symlink escape (layer 2: check_within_root)
// ============================================================================

#[cfg(unix)]
#[test]
fn should_reject_symlink_escaping_crate_root() {
    use std::fs;
    use std::os::unix::fs::symlink;
    use tempfile::TempDir;

    // Create a file outside the future crate root
    let outside = TempDir::new().unwrap();
    let outside_file = outside.path().join("secret.rs");
    fs::write(&outside_file, "// outside crate root").unwrap();

    // Build a minimal crate with src/escape.rs → symlink pointing outside
    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(src.join("lib.rs"), "").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"test-escape\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    symlink(&outside_file, src.join("escape.rs")).unwrap();

    let root_str = root.path().to_str().unwrap_or("");
    let outside_str = outside.path().to_str().unwrap_or("");
    let mut filters: Vec<(&str, &str)> = vec![(root_str, "[ROOT]"), (outside_str, "[OUTSIDE]")];
    filters.extend(backtrace_filters());
    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p").arg(root.path())
            .arg("use")
            .arg("escape"));
    });
}

// ============================================================================
// Broken lib target during root_children top-up — warn, don't hard-fail
// ============================================================================

/// A bin-target query with `-t` triggers the `root_children` top-up, which
/// discovers the *lib* target's module tree even though the query is about
/// the bin. If lib.rs has a syntax error, that discovery fails — the top-up
/// must warn and continue rather than hard-failing an otherwise-healthy
/// query about an unrelated, healthy target.
///
/// Queries a *nested* bin submodule (`main::tool::sub`, `sub` inline inside
/// `tool.rs`) rather than a bare top-level one: for a single-segment
/// normalized path, `compute_root_visibility` unconditionally treats the
/// crate root (which prefers lib.rs) as the parent to look up `mod`
/// visibility in — a separate, pre-existing resolution quirk that would
/// otherwise make the very first, pre-top-up discovery call itself need to
/// parse the broken lib.rs before this test ever reaches the code under
/// test. Nesting one level deeper resolves visibility from `tool.rs`
/// instead, isolating the top-up's own warn-and-continue behavior.
#[test]
fn should_warn_not_fail_when_lib_broken_during_root_children_top_up() {
    use std::fs;
    use tempfile::TempDir;

    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir(&src).unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname = \"test-broken-lib-top-up\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    // Broken: unclosed function signature.
    fs::write(src.join("lib.rs"), "pub fn broken(\n").unwrap();
    fs::write(src.join("main.rs"), "mod tool;\nfn main() {}\n").unwrap();
    fs::write(
        src.join("tool.rs"),
        "pub mod sub {\n    pub fn helper() -> &'static str {\n        \"hello\"\n    }\n}\n",
    )
    .unwrap();

    let root_str = root.path().to_str().unwrap_or("");
    let mut filters: Vec<(&str, &str)> = vec![(root_str, "[ROOT]")];
    filters.extend(backtrace_filters());
    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p").arg(root.path())
            .arg("use")
            .arg("main::tool::sub")
            .arg("-t"));
    });
}
