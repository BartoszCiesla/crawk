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
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/broken_syntax"))
            .arg("use")
            .arg("broken"));
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
