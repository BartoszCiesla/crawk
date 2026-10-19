//! `deps` must analyse every binary and integration-test target from the entry
//! point cargo reports for it, whatever the target's layout and wherever the
//! crate is checked out.
//!
//! Each crate is written into a tempdir instead of living under `fixtures/`: the
//! checkout-path case needs a parent directory a fixture cannot choose, and the
//! layouts would otherwise churn the snapshots of every fixture command. A
//! tempdir path would leak into the header of an `insta` snapshot, so the
//! assertions below read stdout directly.

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use crate::common::crawk_stdout;

const MANIFEST: &str = "[package]\nname = \"my_crate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n";

/// Package `my_crate` made of `files`, each a path relative to the crate root
/// and its contents.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
fn scratch_crate(files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::write(dir.path().join("Cargo.toml"), MANIFEST).expect("write Cargo.toml");
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("create parent");
        fs::write(&path, contents).expect("write source file");
    }
    dir
}

/// Copies the crate at `from` into `to`, leaving out its `target/` directory.
///
/// Test-support scaffolding like [`scratch_crate`], hence the `expect`s.
#[allow(clippy::expect_used)]
fn copy_crate(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create destination");
    for entry in fs::read_dir(from).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let dest = to.join(entry.file_name());
        if path.is_dir() {
            if entry.file_name() != "target" {
                copy_crate(&path, &dest);
            }
        } else {
            fs::copy(&path, &dest).expect("copy file");
        }
    }
}

// A `src/bin/*.rs` binary whose first module alphabetically is a submodule:
// the binary must still be analysed from its own root, so its edges, the
// submodule's included, appear.
#[test]
fn should_deps_binary_with_submodule_from_its_root() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/tool.rs",
            "use my_crate::foo::Bar;\n\
             \n\
             mod inner {\n\
             \x20   use my_crate::foo::Bar;\n\
             \x20   pub fn make() -> Bar { Bar }\n\
             }\n\
             \n\
             fn main() { let _: Bar = inner::make(); }\n",
        ),
    ]);

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps"]),
        "inner -> foo\ntool -> foo\ntool -> inner\n"
    );
}

// A checkout path with a `tests` component must not change the graph: the
// target root comes from cargo, not from the shape of the absolute path.
#[test]
fn should_deps_same_edges_under_a_tests_checkout_path() {
    let fixture = Path::new("fixtures/bin_only");
    let dir = tempfile::tempdir().unwrap();
    let checkout = dir.path().join("tests/app");
    copy_crate(fixture, &checkout);

    assert_eq!(
        crawk_stdout(&checkout, 0, &["deps"]),
        crawk_stdout(fixture, 0, &["deps"])
    );
}

// An integration test named like a library module (`tests/foo.rs`, `src/foo.rs`)
// must be analysed as the test target, not as the library module of that name.
#[test]
fn should_deps_test_target_named_like_a_library_module() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\npub mod bar;\n"),
        ("src/foo.rs", "pub struct Foo;\n"),
        ("src/bar.rs", "pub struct Bar;\n"),
        ("tests/foo.rs", "mod support;\n"),
        (
            "tests/support/mod.rs",
            "use my_crate::bar::Bar;\n#[allow(dead_code)]\npub fn make() -> Bar { Bar }\n",
        ),
    ]);

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps", "-t"]),
        "support -> bar\n"
    );
}
