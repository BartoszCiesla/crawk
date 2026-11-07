//! Where the file of a `mod x;` is looked for: beside a target root, whatever
//! the root file is called, or beside a `mod.rs`; under `<stem>/` for any other
//! file, a `lib.rs` or `main.rs` below a root included. `list`, `use` and `deps`
//! agree on it, and a declaration naming its own file is skipped, not followed
//! forever.
//!
//! The crates are written into tempdirs: a tempdir path would leak into the
//! header of an `insta` snapshot, so the assertions read the output directly.

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::{lines, scratch_crate};

/// Sorted lines of `text`, each with its columns joined by one space.
fn rows(text: &str) -> Vec<String> {
    let mut rows: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    rows.sort_unstable();
    rows
}

/// Package `pkg` whose library root is `src/root.rs` (`[lib] path`).
fn library_root_of_another_name() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"pkg\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                 [lib]\npath = \"src/root.rs\"\n",
            ),
            ("src/root.rs", "pub mod b;\npub mod foo;\n"),
            ("src/b.rs", "use crate::foo::Foo;\npub fn f(_: Foo) {}\n"),
            ("src/foo.rs", "pub struct Foo;\n"),
        ],
    )
}

#[test_case(&["list", "-s"], &["b src/b.rs", "foo src/foo.rs", "lib src/root.rs"]; "list")]
#[test_case(&["deps"], &["b -> foo"]; "deps")]
fn a_library_root_of_another_name_keeps_its_modules(args: &[&str], expected: &[&str]) {
    let krate = library_root_of_another_name();
    assert_eq!(rows(&crawk_stdout(krate.path(), 0, args)), expected);
}

/// `src/a/lib.rs` is the module `a::lib`, whose `mod x;` is `src/a/lib/x.rs`;
/// `src/a/x.rs` is a decoy.
fn nested_lib_rs() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod a;\npub mod foo;\n"),
            ("src/foo.rs", "pub struct Right;\npub struct Wrong;\n"),
            ("src/a.rs", "pub mod lib;\n"),
            ("src/a/lib.rs", "pub mod x;\n"),
            (
                "src/a/lib/x.rs",
                "use crate::foo::Right;\npub fn f(_: Right) {}\n",
            ),
            (
                "src/a/x.rs",
                "use crate::foo::Wrong;\npub fn f(_: Wrong) {}\n",
            ),
        ],
    )
}

#[test_case(
    &["list", "-s"],
    &["a src/a.rs", "a::lib src/a/lib.rs", "a::lib::x src/a/lib/x.rs", "foo src/foo.rs", "lib src/lib.rs"];
    "list"
)]
#[test_case(&["use", "-r", "a::lib"], &["crate::foo::Right"]; "use recursive")]
#[test_case(&["deps", "-a"], &["a::lib::x -> foo [Right]"]; "deps")]
fn a_nested_lib_rs_keeps_its_children_in_its_own_dir(args: &[&str], expected: &[&str]) {
    let krate = nested_lib_rs();
    assert_eq!(rows(&crawk_stdout(krate.path(), 0, args)), expected);
}

/// Valid Rust: `src/a/main.rs` is the module `a::main`, and its own `mod main;`
/// is `src/a/main/main.rs`.
#[test]
fn a_nested_main_rs_declaring_main_is_walked_to_its_end() {
    let krate = scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod a;\n"),
            ("src/a.rs", "pub mod main;\n"),
            ("src/a/main.rs", "pub mod main;\n"),
            ("src/a/main/main.rs", "pub struct X;\n"),
        ],
    );
    assert_eq!(
        rows(&crawk_stdout(krate.path(), 0, &["list", "-s"])),
        [
            "a src/a.rs",
            "a::main src/a/main.rs",
            "a::main::main src/a/main/main.rs",
            "lib src/lib.rs",
        ],
    );
}

/// rustc rejects a root declaring itself (`mod lib;` in `src/lib.rs`) as a
/// circular module; crawk lists the rest of the target instead of crashing.
#[test_case("src/lib.rs", "lib", &["foo", "lib"]; "library root")]
#[test_case("src/main.rs", "main", &["bin@pkg", "bin@pkg::foo"]; "binary root")]
fn a_root_declaring_its_own_file_is_skipped(root: &str, name: &str, expected: &[&str]) {
    let source = format!("pub mod foo;\npub mod {name};\n");
    let krate = scratch_crate("pkg", &[(root, &source), ("src/foo.rs", "")]);
    assert_eq!(lines(&crawk_stdout(krate.path(), 0, &["list"])), expected);
}
