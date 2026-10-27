use std::fs;
use std::path::Path;

use crate::common::{crawk_bin_only, crawk_stdout};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// bin-only crate (no [lib] target) — use command
// ============================================================================

#[test]
fn should_use_main_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main"]));
}

#[test]
fn should_use_main_recursive_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-r"]));
}

#[test]
fn should_use_nested_module_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main::utils::helper"]));
}

#[test]
fn should_use_main_with_expand_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-e"]));
}

#[test]
fn should_use_main_grouped_in_bin_only() {
    assert_cmd_snapshot!(crawk_bin_only().args(["use", "main", "-e", "--format", "grouped"]));
}

/// Writes `contents` to `path` under `root`, creating parent directories.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
fn write_file(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("file has a parent")).expect("create parent");
    fs::write(&path, contents).expect("write file");
}

// Without a library, a path led by the package's own name reaches nothing in
// the package: here a dependency renamed to that name. Both the `use` item and
// the qualified call stay external. Written into a tempdir because the package
// needs a sibling crate to depend on.
#[test]
fn should_use_main_with_own_name_as_external_in_bin_only() {
    let dir = tempfile::tempdir().unwrap();
    let manifest = |name: &str, extra: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{extra}")
    };
    write_file(dir.path(), "helper/Cargo.toml", &manifest("helper", ""));
    write_file(dir.path(), "helper/src/lib.rs", "pub fn greet() {}\n");
    write_file(
        dir.path(),
        "tool/Cargo.toml",
        &manifest(
            "tool",
            "\n[dependencies]\ntool = { package = \"helper\", path = \"../helper\" }\n",
        ),
    );
    write_file(
        dir.path(),
        "tool/src/main.rs",
        "mod config;\n\
         use crate::config::Settings;\n\
         use tool::greet;\n\
         \n\
         fn main() {\n\
         \x20   let _ = Settings;\n\
         \x20   greet();\n\
         \x20   tool::greet();\n\
         }\n",
    );
    write_file(dir.path(), "tool/src/config.rs", "pub struct Settings;\n");

    assert_eq!(
        crawk_stdout(&dir.path().join("tool"), 0, &["use", "main"]),
        "crate::config::Settings\n"
    );
}
