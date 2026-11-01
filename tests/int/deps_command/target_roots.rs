//! `deps` and `list` must discover every binary and integration-test target from
//! the entry point cargo reports for it, whatever the target's layout and
//! wherever the crate is checked out.
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
        "bin@tool -> bin@tool::inner\nbin@tool -> foo\nbin@tool::inner -> foo\n"
    );
}

/// `src/main.rs` and `src/bin/my-tool/main.rs`: two binaries with the stem `main`.
fn main_and_directory_binary() -> TempDir {
    scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/main.rs",
            "use my_crate::foo::Bar;\nfn main() { let _ = Bar; }\n",
        ),
        (
            "src/bin/my-tool/main.rs",
            "mod only_in_tool { pub fn f() {} }\nfn main() { only_in_tool::f(); }\n",
        ),
    ])
}

/// `src/bin/a/main.rs` and `src/bin/b/main.rs`: two binaries with the stem
/// `main` and no `src/main.rs`, each with a module only it declares.
fn two_directory_binaries() -> TempDir {
    scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/a/main.rs",
            "mod only_in_a;\nfn main() { only_in_a::f(); }\n",
        ),
        (
            "src/bin/a/only_in_a.rs",
            "use my_crate::foo::Bar;\npub fn f() { let _ = Bar; }\n",
        ),
        (
            "src/bin/b/main.rs",
            "mod only_in_b { pub fn f() {} }\nfn main() { only_in_b::f(); }\n",
        ),
    ])
}

// Binaries sharing a file stem: each is read from its own file, so the second
// one's rows carry its source and the module only it declares.
#[test]
fn should_list_binary_sharing_a_stem_from_its_own_file() {
    let dir = main_and_directory_binary();

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["list", "-T", "-s"]),
        "[lib]          foo                        src/foo.rs\n\
         [lib]          lib                        src/lib.rs\n\
         [bin@my-tool]  bin@my-tool                src/bin/my-tool/main.rs\n\
         [bin@my-tool]  bin@my-tool::only_in_tool  src/bin/my-tool/main.rs\n\
         [bin@my_crate] bin@my_crate               src/main.rs\n"
    );
}

// Two nodes, each with its own edges, though both files have the stem `main`.
#[test]
fn should_deps_binary_sharing_a_stem_from_its_own_file() {
    let dir = main_and_directory_binary();

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps"]),
        "bin@my-tool -> bin@my-tool::only_in_tool\nbin@my_crate -> foo\n"
    );
}

#[test]
fn should_list_two_directory_binaries_from_their_own_files() {
    let dir = two_directory_binaries();

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["list", "-T", "-s"]),
        "[lib]   foo               src/foo.rs\n\
         [lib]   lib               src/lib.rs\n\
         [bin@a] bin@a             src/bin/a/main.rs\n\
         [bin@a] bin@a::only_in_a  src/bin/a/only_in_a.rs\n\
         [bin@b] bin@b             src/bin/b/main.rs\n\
         [bin@b] bin@b::only_in_b  src/bin/b/main.rs\n"
    );
}

#[test]
fn should_deps_two_directory_binaries_from_their_own_files() {
    let dir = two_directory_binaries();

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps"]),
        "bin@a -> bin@a::only_in_a\nbin@a::only_in_a -> foo\nbin@b -> bin@b::only_in_b\n"
    );
}

// A binary's root is a crate root whatever its file is called: rustc looks for
// `mod shared;` of `src/bin/tool.rs` beside the file, not in `src/bin/tool/`.
#[test]
fn should_resolve_module_of_a_binary_root_beside_its_file() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/tool.rs",
            "mod shared;\nfn main() { shared::f(); }\n",
        ),
        (
            "src/bin/shared/mod.rs",
            "use my_crate::foo::Bar;\npub fn f() { let _ = Bar; }\n",
        ),
    ]);

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["list", "-T", "-s"]),
        "[lib]      foo               src/foo.rs\n\
         [lib]      lib               src/lib.rs\n\
         [bin@tool] bin@tool          src/bin/tool.rs\n\
         [bin@tool] bin@tool::shared  src/bin/shared/mod.rs\n"
    );
    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps"]),
        "bin@tool -> bin@tool::shared\nbin@tool::shared -> foo\n"
    );
}

// A binary named like the package is the default for `src/bin/<package>.rs`.
// Its cargo name is a library alias too, but the entry point is its own file.
#[test]
fn should_not_read_binary_named_like_the_crate_from_the_library() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/my_crate.rs",
            "mod only_in_bin { pub fn f() {} }\nfn main() { only_in_bin::f(); }\n",
        ),
    ]);

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["list", "-T", "-s"]),
        "[lib]          foo                        src/foo.rs\n\
         [lib]          lib                        src/lib.rs\n\
         [bin@my_crate] bin@my_crate               src/bin/my_crate.rs\n\
         [bin@my_crate] bin@my_crate::only_in_bin  src/bin/my_crate.rs\n"
    );
}

// `src/bin/helper.rs` is a binary and a module of `tool`, with an inline child.
// Seen from `tool` that child's root is the module `helper`, not a crate root,
// so `tool` must not be lost to a failed lookup of `mod helper` inside the file.
#[test]
fn should_deps_module_whose_file_is_also_a_binary() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/tool.rs",
            "use my_crate::foo::Bar;\nmod helper;\nfn main() { let _ = Bar; helper::f(); }\n",
        ),
        (
            "src/bin/helper.rs",
            "use my_crate::foo::Bar;\n\
             mod deep { pub fn x() {} }\n\
             pub fn f() { let _ = Bar; deep::x(); }\n\
             fn main() {}\n",
        ),
    ]);

    assert_eq!(
        crawk_stdout(dir.path(), 0, &["deps"]),
        "bin@helper -> bin@helper::deep\n\
         bin@helper -> foo\n\
         bin@tool -> bin@tool::helper\n\
         bin@tool -> foo\n\
         bin@tool::helper -> bin@tool::helper::deep\n\
         bin@tool::helper -> foo\n"
    );
}

// An inline module named like its file is still inline: asking for it must not
// analyse the whole file.
#[test]
fn should_use_inline_module_named_like_its_binary_file() {
    let dir = scratch_crate(&[
        ("src/lib.rs", "pub mod foo;\n"),
        ("src/foo.rs", "pub struct Bar;\n"),
        (
            "src/bin/tool.rs",
            "use my_crate::foo::Bar;\n\
             mod tool { pub mod inner { pub fn f() {} } }\n\
             fn main() { let _ = Bar; tool::inner::f(); }\n",
        ),
    ]);

    assert_eq!(crawk_stdout(dir.path(), 0, &["use", "tool::tool"]), "");
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
