//! An integration-test target goes by `test@<cargo target name>`, its modules by
//! `test@<name>::<path>`, in every command with `-t` and in `check -t` rules: the
//! rule binaries follow (`bin_names.rs`).
//!
//! The file stem names nothing (`tests/mod.rs` would be `mod`, `tests/foo/main.rs`
//! would be `main`), and a test target stays apart from a library module named
//! alike (`tests/cli.rs` next to `src/cli.rs`). The crates are written into
//! tempdirs: a tempdir path would leak into the header of an `insta` snapshot, so
//! the assertions read the output directly.

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::{crawk_failure, crawk_output, lines, scratch_crate, write_rules};

/// Target `integration` with its entry point at `tests/mod.rs`, the way crawk's
/// own test target is laid out: the stem says `mod`, cargo says `integration`.
fn renamed_entry_point() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"pkg\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                 [[test]]\nname = \"integration\"\npath = \"tests/mod.rs\"\n",
            ),
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo.rs", "pub struct Foo;\n"),
            (
                "tests/mod.rs",
                "mod common;\nuse pkg::foo::Foo;\n#[test]\nfn t() { common::f(); let _ = Foo; }\n",
            ),
            ("tests/common/mod.rs", "pub fn f() {}\n"),
        ],
    )
}

/// `tests/foo/main.rs`, cargo's directory layout for tests: target `foo`, stem
/// `main`, the same stem as the binary `src/main.rs`.
fn directory_test_beside_a_binary() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod a;\npub mod b;\n"),
            ("src/a.rs", "pub struct A;\n"),
            ("src/b.rs", "pub struct B;\n"),
            ("src/main.rs", "use pkg::a::A;\nfn main() { let _ = A; }\n"),
            (
                "tests/foo/main.rs",
                "use pkg::b::B;\n#[test]\nfn t() { let _ = B; }\n",
            ),
        ],
    )
}

/// `tests/cli.rs` next to a library module `cli`; the test reaches that module,
/// and the module reaches another one, so the two `cli` have different edges.
fn test_named_like_a_library_module() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod cli;\npub mod other;\n"),
            (
                "src/cli.rs",
                "use crate::other::Other;\npub fn run(_: Other) {}\n",
            ),
            ("src/other.rs", "pub struct Other;\n"),
            (
                "tests/cli.rs",
                "use pkg::cli::run;\n#[test]\nfn t() { let _ = run; }\n",
            ),
        ],
    )
}

/// `tests/it/main.rs` declares a module `shared`, like the library does, and has
/// an item at its root. The test module names the library's `other` bare.
fn test_module_named_like_a_library_module() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod other;\npub mod shared;\n"),
            ("src/other.rs", "pub fn f() {}\n"),
            ("src/shared.rs", "pub struct InLib;\n"),
            (
                "tests/it/main.rs",
                "mod shared;\npub struct AtRoot;\n#[test]\nfn t() { shared::f(); }\n",
            ),
            (
                "tests/it/shared.rs",
                "use crate::AtRoot;\nuse pkg::shared::InLib;\n\
                 pub fn f() { let _ = (AtRoot, InLib); other::f(); }\n",
            ),
        ],
    )
}

// ---- the three cases of the naming decision ----

#[test]
fn a_test_root_goes_by_its_cargo_name_not_its_entry_file() {
    let krate = renamed_entry_point();
    let stdout = crawk_stdout(krate.path(), 0, &["list", "-t", "-T"]);
    let rows: Vec<Vec<&str>> = stdout
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        rows,
        [
            ["[lib]", "foo"],
            ["[lib]", "lib"],
            ["[test@integration]", "test@integration"],
            ["[test@integration]", "test@integration::common"],
        ],
    );
}

#[test]
fn the_stem_of_a_test_file_points_at_its_target() {
    let krate = renamed_entry_point();
    let stderr = crawk_failure(krate.path(), 1, &["use", "mod", "-t"]);
    assert!(stderr.contains("test@integration"), "{stderr}");
}

#[test]
fn a_stem_shared_with_a_binary_points_at_both() {
    let krate = directory_test_beside_a_binary();
    let stderr = crawk_failure(krate.path(), 1, &["use", "main", "-t"]);
    assert!(stderr.contains("bin@pkg, test@foo"), "{stderr}");
}

#[test]
fn without_tests_a_stem_points_only_at_the_binary() {
    let krate = directory_test_beside_a_binary();
    let stderr = crawk_failure(krate.path(), 1, &["use", "main"]);
    assert!(stderr.contains("bin@pkg"), "{stderr}");
    assert!(!stderr.contains("test@"), "{stderr}");
}

#[test_case("test@foo"; "full name")]
#[test_case("foo"; "bare cargo name")]
fn use_reads_the_directory_test_not_the_binary(query: &str) {
    let krate = directory_test_beside_a_binary();
    let stdout = crawk_stdout(krate.path(), 0, &["use", query, "-t"]);
    assert_eq!(stdout, "pkg::b::B\n");
}

#[test]
fn a_bare_name_of_a_library_module_and_a_test_lists_both() {
    let krate = test_named_like_a_library_module();
    let stderr = crawk_failure(krate.path(), 1, &["use", "cli", "-t"]);
    assert!(stderr.contains("lib::cli, test@cli"), "{stderr}");
}

#[test]
fn without_tests_a_bare_name_means_the_library_module() {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["use", "cli"]);
    assert_eq!(stdout, "crate::other::Other\n");
}

#[test_case("test@cli", "pkg::cli::run\n"; "the test")]
#[test_case("lib::cli", "crate::other::Other\n"; "the library module")]
fn a_full_name_picks_one_of_the_two(query: &str, expected: &str) {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["use", query, "-t"]);
    assert_eq!(stdout, expected);
}

// ---- every command, one name ----

#[test]
fn list_shows_the_full_name_in_every_form() {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["list", "test@cli", "-t"]);
    assert_eq!(stdout, "test@cli\n");

    let krate = test_module_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["list", "it", "-t"]);
    assert_eq!(lines(&stdout), ["test@it", "test@it::shared"]);
}

#[test]
fn a_full_name_needs_tests_to_be_included() {
    let krate = test_named_like_a_library_module();
    let stderr = crawk_failure(krate.path(), 1, &["use", "test@cli"]);
    assert!(stderr.contains("test@cli"), "{stderr}");
}

#[test]
fn deps_keeps_a_test_apart_from_a_library_module_named_alike() {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-t"]);
    assert_eq!(lines(&stdout), ["cli -> other", "test@cli -> cli"]);
}

#[test]
fn deps_keeps_same_named_modules_apart_and_finds_crate_paths_in_the_test() {
    let krate = test_module_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-t"]);
    assert_eq!(
        lines(&stdout),
        [
            "test@it -> test@it::shared",
            "test@it::shared -> shared",
            "test@it::shared -> test@it",
        ]
    );
}

#[test]
fn deps_depth_collapses_a_test_onto_its_root() {
    let krate = test_module_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-t", "--depth", "1"]);
    assert_eq!(stdout, "test@it -> shared\n");
}

#[test]
fn deps_path_accepts_test_names() {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(
        krate.path(),
        0,
        &["deps", "-t", "--path", "test@cli", "other"],
    );
    assert_eq!(stdout, "test@cli -> cli -> other\n");
}

#[test]
fn why_explains_the_addressed_test() {
    let krate = test_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["why", "test@cli", "lib::cli", "-t"]);
    assert!(stdout.contains("pkg::cli::run"), "{stdout}");
}

#[test]
fn why_resolves_crate_paths_inside_the_source_test() {
    let krate = test_module_named_like_a_library_module();
    let stdout = crawk_stdout(
        krate.path(),
        0,
        &["why", "test@it::shared", "test@it", "-t"],
    );
    assert!(stdout.contains("crate::AtRoot"), "{stdout}");
}

// A test module is a compilation unit apart from the library, however it is
// named: the library's top-level modules are not in its scope.
#[test]
fn a_test_module_does_not_inherit_the_library_root_children() {
    let krate = test_module_named_like_a_library_module();
    let stdout = crawk_stdout(krate.path(), 0, &["use", "test@it::shared", "-t"]);
    assert_eq!(lines(&stdout), ["crate::AtRoot", "pkg::shared::InLib"]);
}

#[test]
fn a_bare_name_of_a_binary_and_a_test_lists_both_with_tests_only() {
    let krate = scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", ""),
            ("src/bin/x.rs", "fn main() {}\n"),
            ("tests/x.rs", ""),
        ],
    );
    let stderr = crawk_failure(krate.path(), 1, &["use", "x", "-t"]);
    assert!(stderr.contains("bin@x, test@x"), "{stderr}");
    crawk_stdout(krate.path(), 0, &["use", "x"]);
}

#[test]
fn a_rule_takes_the_full_name() {
    let krate = test_named_like_a_library_module();
    write_rules(
        krate.path(),
        "[[check.deny]]\nfrom = \"test@cli\"\nto = \"cli\"\n",
    );
    let output = crawk_output(krate.path(), &["check", "-t"]);
    assert_eq!(output.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("test@cli -> cli"), "{stdout}");
}

#[test]
fn a_rule_on_the_library_module_does_not_cover_the_test_named_alike() {
    let krate = test_named_like_a_library_module();
    write_rules(
        krate.path(),
        "[[check.deny]]\nfrom = \"cli\"\nto = \"cli\"\n",
    );
    let output = crawk_output(krate.path(), &["check", "-t"]);
    assert_eq!(output.status.code(), Some(0));
}

#[test]
fn a_rule_with_a_bare_test_name_says_what_it_may_have_meant() {
    let krate = renamed_entry_point();
    write_rules(
        krate.path(),
        "[[check.deny]]\nfrom = \"common\"\nto = \"foo\"\n",
    );
    let stderr = crawk_failure(krate.path(), 2, &["check", "-t"]);
    assert!(stderr.contains("test@integration::common"), "{stderr}");
}

#[test]
fn a_rule_on_a_stem_points_at_the_test() {
    let krate = renamed_entry_point();
    write_rules(
        krate.path(),
        "[[check.deny]]\nfrom = \"mod\"\nto = \"foo\"\n",
    );
    let stderr = crawk_failure(krate.path(), 2, &["check", "-t"]);
    assert!(stderr.contains("test@integration"), "{stderr}");
}

#[test]
fn a_rule_on_a_test_needs_tests_to_be_included() {
    let krate = test_named_like_a_library_module();
    write_rules(
        krate.path(),
        "[[check.deny]]\nfrom = \"test@cli\"\nto = \"cli\"\n",
    );
    crawk_failure(krate.path(), 2, &["check"]);
}
