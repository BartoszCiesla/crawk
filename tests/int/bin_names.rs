//! A binary target goes by `bin@<cargo target name>`, its modules by
//! `bin@<name>::<path>`, in every command and in `check` rules.
//!
//! Binaries sharing a file stem (`src/main.rs` + `src/bin/my-tool/main.rs`) and
//! same-named modules across targets must stay apart. The crates are written
//! into tempdirs: a tempdir path would leak into the header of an `insta`
//! snapshot, so the assertions read the output directly.

use std::fs;
use std::path::Path;
use std::process::Output;

use tempfile::TempDir;
use test_case::test_case;

use crate::common::{crawk, crawk_stdout};

/// Package `name` made of `files`, each a path relative to the crate root and
/// its contents.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
pub(super) fn scratch_crate(name: &str, files: &[(&str, &str)]) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest =
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n");
    fs::write(dir.path().join("Cargo.toml"), manifest).expect("write Cargo.toml");
    for (path, contents) in files {
        let path = dir.path().join(path);
        fs::create_dir_all(path.parent().expect("file has a parent")).expect("create parent");
        fs::write(&path, contents).expect("write source file");
    }
    dir
}

/// `src/main.rs` (target `my-crate`, like the package) and
/// `src/bin/my-tool/main.rs` (target `my-tool`): both have the stem `main`.
fn stem_collision() -> TempDir {
    scratch_crate(
        "my-crate",
        &[
            ("src/lib.rs", "pub mod foo;\n"),
            ("src/foo.rs", "pub struct Bar;\npub struct Baz;\n"),
            (
                "src/main.rs",
                "use my_crate::foo::Bar;\nfn main() { let _ = Bar; }\n",
            ),
            (
                "src/bin/my-tool/main.rs",
                "use my_crate::foo::Baz;\n\
                 mod only_in_tool { pub fn f() {} }\n\
                 fn main() { let _ = Baz; only_in_tool::f(); }\n",
            ),
        ],
    )
}

/// `src/bin/a/main.rs` and `src/bin/b/main.rs` each declare their own `shared`,
/// beside a library module `shared` and a library module `a` named like a binary.
fn two_directory_binaries() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub mod a;\npub mod shared;\n"),
            ("src/a.rs", "pub struct InLibA;\n"),
            (
                "src/shared.rs",
                "pub struct InLibShared;\npub mod deep { pub struct Wrong; }\n",
            ),
            (
                "src/bin/a/main.rs",
                "mod shared;\npub struct AtRoot;\nfn main() { shared::f(); }\n",
            ),
            (
                "src/bin/a/shared.rs",
                "use pkg::a::InLibA;\nuse crate::AtRoot;\n\
                 pub fn f() { let _ = (InLibA, AtRoot); }\n",
            ),
            (
                "src/bin/b/main.rs",
                "mod shared;\nuse crate::shared::*;\nfn main() { f(); }\n",
            ),
            (
                "src/bin/b/shared.rs",
                "pub fn f() { deep::g(); }\n\
                 pub mod deep { use pkg::shared::InLibShared; \
                 pub fn g() { let _ = InLibShared; } }\n",
            ),
        ],
    )
}

/// Runs crawk on `root` and returns the whole output.
#[allow(clippy::expect_used)]
pub(super) fn crawk_output(root: &Path, args: &[&str]) -> Output {
    crawk()
        .arg("-p")
        .arg(root)
        .args(args)
        .output()
        .expect("run crawk")
}

/// Runs a crawk command that must fail with `code` and returns its stderr.
pub(super) fn crawk_failure(root: &Path, code: i32, args: &[&str]) -> String {
    let output = crawk_output(root, args);
    assert_eq!(output.status.code(), Some(code), "crawk {args:?}");
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Sorted, trimmed lines of `text`.
pub(super) fn lines(text: &str) -> Vec<&str> {
    let mut lines: Vec<&str> = text.lines().map(str::trim_end).collect();
    lines.sort_unstable();
    lines
}

/// Writes a rule config beside the crate.
#[allow(clippy::expect_used)]
pub(super) fn write_rules(root: &Path, rules: &str) {
    fs::write(root.join("crawk.toml"), rules).expect("write crawk.toml");
}

/// Writes a rule config beside the crate and runs `check` on it.
fn check(root: &Path, rules: &str) -> Output {
    write_rules(root, rules);
    crawk_output(root, &["check"])
}

#[test]
fn list_gives_each_binary_of_a_stem_its_own_rows() {
    let krate = stem_collision();
    let stdout = crawk_stdout(krate.path(), 0, &["list", "-T", "-s"]);
    let rows: Vec<Vec<&str>> = stdout
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert_eq!(
        rows,
        [
            ["[lib]", "foo", "src/foo.rs"],
            ["[lib]", "lib", "src/lib.rs"],
            ["[bin@my-crate]", "bin@my-crate", "src/main.rs"],
            ["[bin@my-tool]", "bin@my-tool", "src/bin/my-tool/main.rs"],
            [
                "[bin@my-tool]",
                "bin@my-tool::only_in_tool",
                "src/bin/my-tool/main.rs"
            ],
        ],
    );
}

#[test_case("bin@my-tool", &["bin@my-tool", "bin@my-tool::only_in_tool"]; "root")]
#[test_case("bin@my-tool::only_in_tool", &["bin@my-tool::only_in_tool"]; "inline module")]
#[test_case("bin@my-crate", &["bin@my-crate"]; "binary named like the package")]
fn list_of_a_binary_path_stays_in_that_binary(query: &str, expected: &[&str]) {
    let krate = stem_collision();
    let stdout = crawk_stdout(krate.path(), 0, &["list", query]);
    assert_eq!(lines(&stdout), expected);
}

#[test_case("bin@my-crate", "my_crate::foo::Bar\n"; "default binary")]
#[test_case("bin@my-tool", "crate::only_in_tool::f\nmy_crate::foo::Baz\n"; "directory binary")]
fn use_reads_the_addressed_binary(query: &str, expected: &str) {
    let krate = stem_collision();
    assert_eq!(crawk_stdout(krate.path(), 0, &["use", query]), expected);
}

#[test_case("bin@a::shared", &["crate::AtRoot", "pkg::a::InLibA"]; "module of the first binary")]
#[test_case("bin@b::shared", &["crate::shared::deep::g"]; "file level of the second binary")]
#[test_case("bin@b::shared::deep", &["pkg::shared::InLibShared"]; "inline module of the second binary")]
fn use_keeps_same_named_modules_apart(query: &str, expected: &[&str]) {
    let krate = two_directory_binaries();
    let stdout = crawk_stdout(krate.path(), 0, &["use", query]);
    assert_eq!(lines(&stdout), expected);
}

#[test]
fn use_recursive_finds_modules_beside_a_named_entry_file() {
    let krate = scratch_crate(
        "pkg",
        &[
            ("src/lib.rs", "pub struct Item;\n"),
            ("src/bin/tool.rs", "mod helper;\nfn main() {}\n"),
            ("src/bin/helper.rs", "use pkg::Item;\n"),
        ],
    );
    let stdout = crawk_stdout(krate.path(), 0, &["use", "-r", "bin@tool"]);
    assert_eq!(stdout, "pkg::Item\n");
}

#[test]
fn glob_in_a_binary_resolves_to_its_own_module() {
    let krate = two_directory_binaries();
    let stdout = crawk_stdout(krate.path(), 0, &["use", "-G", "bin@b"]);
    assert_eq!(lines(&stdout), ["crate::shared::deep", "crate::shared::f"]);
}

#[test_case("bin@my-crate", "my_crate::foo::Bar"; "default binary")]
#[test_case("bin@my-tool", "my_crate::foo::Baz"; "directory binary")]
fn why_explains_the_addressed_binary(source: &str, reference: &str) {
    let krate = stem_collision();
    let stdout = crawk_stdout(krate.path(), 0, &["why", source, "foo"]);
    assert!(stdout.contains(reference), "{stdout}");
    assert_eq!(stdout.lines().count(), 1, "{stdout}");
}

#[test]
fn why_resolves_crate_paths_inside_the_source_binary() {
    let krate = two_directory_binaries();
    let stdout = crawk_stdout(krate.path(), 0, &["why", "bin@a::shared", "bin@a"]);
    assert!(stdout.contains("crate::AtRoot"), "{stdout}");
    let unrelated = crawk_stdout(krate.path(), 0, &["why", "bin@a::shared", "lib::shared"]);
    assert_eq!(unrelated, "");
}

#[test]
fn deps_has_one_node_per_binary_with_its_own_edges() {
    let krate = stem_collision();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["deps"]),
        "bin@my-crate -> foo\n\
         bin@my-tool -> bin@my-tool::only_in_tool\n\
         bin@my-tool -> foo\n",
    );
}

#[test]
fn deps_keeps_same_named_modules_and_a_binary_named_like_a_module_apart() {
    let krate = two_directory_binaries();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["deps"]),
        "bin@a -> bin@a::shared\n\
         bin@a::shared -> a\n\
         bin@a::shared -> bin@a\n\
         bin@b -> bin@b::shared\n\
         bin@b::shared -> bin@b::shared::deep\n\
         bin@b::shared::deep -> shared\n",
    );
}

#[test]
fn deps_depth_collapses_a_binary_onto_its_root() {
    let krate = two_directory_binaries();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["deps", "-d", "1"]),
        "bin@a -> a\nbin@b -> shared\n",
    );
}

#[test]
fn deps_dot_quotes_binary_nodes() {
    let krate = stem_collision();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-f", "dot"]);
    assert!(stdout.contains(r#""bin@my-tool" -> "foo";"#), "{stdout}");
}

#[test]
fn deps_path_accepts_binary_names() {
    let krate = stem_collision();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["deps", "--path", "bin@my-tool", "foo"]),
        "bin@my-tool -> foo\n",
    );
}

#[test]
fn a_bare_cargo_name_reaches_the_binary() {
    let krate = stem_collision();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["use", "my-tool"]),
        crawk_stdout(krate.path(), 0, &["use", "bin@my-tool"]),
    );
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["list", "my-tool::only_in_tool"]),
        "bin@my-tool::only_in_tool\n",
    );
}

#[test]
fn a_bare_unique_module_is_listed_under_its_full_name() {
    let krate = stem_collision();
    assert_eq!(
        crawk_stdout(krate.path(), 0, &["list", "only_in_tool"]),
        "bin@my-tool::only_in_tool\n",
    );
}

#[test]
fn why_takes_bare_unique_paths_on_both_sides() {
    let krate = scratch_crate(
        "pkg",
        &[
            ("src/main.rs", "mod a;\nmod b;\nfn main() {}\n"),
            ("src/a.rs", "use crate::b::B;\npub fn f(_: B) {}\n"),
            ("src/b.rs", "pub struct B;\n"),
        ],
    );
    let stdout = crawk_stdout(krate.path(), 0, &["why", "a", "b", "-f", "grouped"]);
    assert!(stdout.contains("crate::b::B"), "{stdout}");
    assert!(stdout.contains("bin@pkg::a"), "{stdout}");
}

#[test_case(&["use", "shared"]; "use")]
#[test_case(&["list", "shared"]; "list")]
#[test_case(&["why", "shared", "a"]; "why source")]
#[test_case(&["why", "bin@a", "shared"]; "why target")]
#[test_case(&["deps", "--path", "shared", "a"]; "deps path")]
fn a_bare_name_of_several_modules_lists_them(args: &[&str]) {
    let krate = two_directory_binaries();
    let stderr = crawk_failure(krate.path(), 1, args);
    assert!(
        stderr.contains("lib::shared, bin@a::shared, bin@b::shared"),
        "{stderr}"
    );
}

#[test]
fn a_bare_name_of_a_library_module_and_a_binary_lists_both() {
    let krate = two_directory_binaries();
    let stderr = crawk_failure(krate.path(), 1, &["use", "a"]);
    assert!(stderr.contains("lib::a, bin@a"), "{stderr}");
    let stdout = crawk_stdout(krate.path(), 0, &["list", "lib::a"]);
    assert_eq!(stdout, "a\n");
}

#[test]
fn a_file_stem_points_at_the_binaries_that_have_it() {
    let krate = stem_collision();
    let stderr = crawk_failure(krate.path(), 1, &["use", "main"]);
    assert!(stderr.contains("bin@my-crate, bin@my-tool"), "{stderr}");
}

#[test]
fn the_package_name_means_the_library_not_the_binary_named_alike() {
    let krate = scratch_crate(
        "my-crate",
        &[
            ("src/lib.rs", "pub mod foo;\nuse crate::foo::Bar as _;\n"),
            ("src/foo.rs", "pub struct Bar;\n"),
            (
                "src/main.rs",
                "use my_crate::foo::Bar;\nfn main() { let _ = Bar; }\n",
            ),
        ],
    );
    let library = crawk_stdout(krate.path(), 0, &["use", "my-crate"]);
    assert_eq!(library, crawk_stdout(krate.path(), 0, &["use", "lib"]));
    assert!(library.contains("crate::foo::Bar"), "{library}");
    let binary = crawk_stdout(krate.path(), 0, &["use", "bin@my-crate"]);
    assert_eq!(binary, "my_crate::foo::Bar\n");
}

#[test]
fn without_a_library_the_package_name_means_the_only_binary() {
    let krate = scratch_crate(
        "pkg",
        &[
            ("src/main.rs", "mod a;\nfn main() { a::f(); }\n"),
            ("src/a.rs", "pub fn f() {}\n"),
        ],
    );
    assert_eq!(
        lines(&crawk_stdout(krate.path(), 0, &["list", "pkg"])),
        ["bin@pkg", "bin@pkg::a"],
    );
}

#[test_case("from = \"bin@my-tool\"", 1, "bin@my-tool -> foo"; "the named binary")]
#[test_case("from = \"bin@my-crate\"", 1, "bin@my-crate -> foo"; "the binary named like the package")]
fn deny_on_a_binary_hits_only_that_binary(from: &str, code: i32, violation: &str) {
    let krate = stem_collision();
    let output = check(
        krate.path(),
        &format!("[check]\n[[check.deny]]\n{from}\nto = \"foo\"\n"),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(code), "{stdout}");
    assert_eq!(stdout.matches("DENY").count(), 1, "{stdout}");
    assert!(stdout.contains(violation), "{stdout}");
}

#[test]
fn restrict_on_a_binary_exempts_its_own_subtree_and_spares_the_other() {
    let krate = stem_collision();
    let output = check(
        krate.path(),
        "[check]\n[[check.restrict]]\nfrom = \"bin@my-tool\"\nto = []\n",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("bin@my-tool -> foo"), "{stdout}");
    assert!(!stdout.contains("only_in_tool"), "{stdout}");
    assert!(!stdout.contains("bin@my-crate"), "{stdout}");
}

#[test]
fn subtree_rule_on_a_binary_covers_its_modules_only() {
    let krate = two_directory_binaries();
    let output = check(
        krate.path(),
        "[check]\n[[check.deny]]\nfrom = \"bin@a::*\"\nto = \"a\"\n",
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("bin@a::shared -> a"), "{stdout}");
    assert!(!stdout.contains("bin@b"), "{stdout}");
}

#[test_case("main", "bin@my-crate, bin@my-tool"; "file stem")]
#[test_case("my-tool", "bin@my-tool"; "bare cargo name")]
#[test_case("only_in_tool", "bin@my-tool::only_in_tool"; "bare module")]
#[test_case("only_in_tool::*", "bin@my-tool::only_in_tool"; "bare subtree")]
fn a_rule_must_use_the_full_name_and_the_error_says_which(from: &str, hint: &str) {
    let krate = stem_collision();
    write_rules(
        krate.path(),
        &format!("[check]\n[[check.deny]]\nfrom = \"{from}\"\nto = \"foo\"\n"),
    );
    let stderr = crawk_failure(krate.path(), 2, &["check"]);
    assert!(stderr.contains("unknown module"), "{stderr}");
    assert!(stderr.contains(hint), "{stderr}");
}

#[test]
fn an_unknown_binary_is_not_found() {
    let krate = stem_collision();
    let stderr = crawk_failure(krate.path(), 1, &["use", "bin@nope"]);
    assert!(stderr.contains("Module 'bin@nope' not found"), "{stderr}");
}

#[test_case("bin@"; "no name")]
#[test_case("foo::bin@my-tool"; "not leading")]
#[test_case("lib@foo"; "other kind")]
#[test_case("bin@a@b"; "two at signs")]
#[test_case("bin:my-tool"; "single colon")]
fn a_malformed_binary_path_is_rejected_by_the_cli(path: &str) {
    let krate = stem_collision();
    let stderr = crawk_failure(krate.path(), 2, &["use", path]);
    assert!(stderr.contains("malformed"), "{stderr}");
}
