//! The library's root goes by `lib`, its modules by their bare path, and a root
//! alias (`lib`, the crate name, the package name) may lead a path typed on the
//! command line.
//!
//! A library module named like an alias stays reachable: the name crawk prints
//! for it reads back as that module (`my_crate::inner`), so does the spelling led
//! by an alias (`lib::my_crate::inner`), and `lib` alone is the root. A top-level
//! module named `lib` goes by `lib::lib`, its children by `lib::lib::<path>`, so
//! it never shares a node with the root. The crates
//! are written into tempdirs: a tempdir path would leak into the header of an
//! `insta` snapshot, so the assertions read the output directly.

use std::fs;

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::{crawk_output, lines, scratch_crate, write_rules};

/// Package `my-crate` with a module `my_crate`, named like the crate, which has
/// an inline `inner` beside a top-level `inner`.
fn module_named_like_the_crate() -> TempDir {
    scratch_crate(
        "my-crate",
        &[
            (
                "src/lib.rs",
                "pub mod foo;\npub mod inner;\npub mod my_crate;\n\
                 pub mod other {\n    use crate::my_crate::inner::*;\n    use crate::my_crate::*;\n    \
                 pub fn k() { h(); q(); }\n}\n",
            ),
            (
                "src/foo.rs",
                "pub struct Baz;\npub struct Qux;\npub struct Top;\n",
            ),
            (
                "src/inner.rs",
                "use crate::foo::Top;\npub fn t() -> Top { Top }\n",
            ),
            (
                "src/my_crate.rs",
                "use crate::foo::Qux;\npub fn q() -> Qux { Qux }\n\
                 pub mod inner {\n    use crate::foo::Baz;\n    pub fn h() -> Baz { Baz }\n}\n",
            ),
        ],
    )
}

#[test_case(&["use", "my_crate"], "crate::foo::Qux\n"; "use by its name")]
#[test_case(&["use", "lib::my_crate"], "crate::foo::Qux\n"; "use led by lib")]
#[test_case(&["use", "my_crate::inner"], "crate::foo::Baz\n"; "use child by its name")]
#[test_case(&["use", "lib::my_crate::inner"], "crate::foo::Baz\n"; "use child led by lib")]
#[test_case(&["use", "my_crate::my_crate::inner"], "crate::foo::Baz\n"; "use child led by crate name")]
#[test_case(&["use", "my-crate::my_crate::inner"], "crate::foo::Baz\n"; "use child led by package name")]
#[test_case(&["use", "inner"], "crate::foo::Top\n"; "use top level namesake")]
#[test_case(&["use", "lib::inner"], "crate::foo::Top\n"; "use top level namesake led by lib")]
#[test_case(&["why", "my_crate", "foo"], "crate::foo::Qux\n"; "why source")]
#[test_case(&["why", "lib::my_crate::inner", "foo"], "crate::foo::Baz\n"; "why child source")]
#[test_case(&["why", "other", "my_crate::inner"], "crate::my_crate::inner::*\n"; "why target")]
#[test_case(&["deps", "--path", "other", "my_crate::inner"], "other -> my_crate::inner\n"; "deps path")]
#[test_case(&["list", "my_crate"], "my_crate\nmy_crate::inner\n"; "list by its name")]
#[test_case(&["list", "lib::my_crate"], "my_crate\nmy_crate::inner\n"; "list led by lib")]
fn a_module_named_like_the_crate_is_reachable(args: &[&str], expected: &str) {
    let krate = module_named_like_the_crate();
    assert_eq!(crawk_stdout(krate.path(), 0, args), expected);
}

#[test_case("lib"; "lib")]
#[test_case("my-crate"; "package name")]
fn a_root_alias_alone_is_the_root(root: &str) {
    let krate = module_named_like_the_crate();
    assert_eq!(
        lines(&crawk_stdout(krate.path(), 0, &["list", root])),
        ["foo", "inner", root, "my_crate", "my_crate::inner", "other"],
    );
}

#[test]
fn a_glob_into_a_module_named_like_the_crate_resolves() {
    let krate = module_named_like_the_crate();
    assert_eq!(
        lines(&crawk_stdout(krate.path(), 0, &["use", "-G", "other"])),
        [
            "crate::my_crate::inner",
            "crate::my_crate::inner::h",
            "crate::my_crate::q",
        ],
    );
}

/// Package `pkg` with an inline `mod lib` in `src/lib.rs`, named like the root
/// alias. Its children `inner` and `other` have top-level namesakes, and `other`
/// reaches both the root (a re-export) and the module.
fn inline_module_named_lib() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            (
                "src/lib.rs",
                "pub mod foo;\npub mod inner;\npub use foo::Top as Reexported;\n\
                 pub mod lib {\n    use crate::foo::Bar;\n    pub fn g() -> Bar { Bar }\n    \
                 pub mod inner {\n        use crate::foo::Baz;\n        pub fn f() -> Baz { Baz }\n    }\n    \
                 pub mod other {}\n}\n\
                 pub mod other {\n    use crate::lib::inner::*;\n    use crate::lib::g;\n    \
                 use crate::Reexported;\n    pub fn k() { f(); let _ = (g(), Reexported); }\n}\n",
            ),
            (
                "src/foo.rs",
                "pub struct Bar;\npub struct Baz;\npub struct Top;\n",
            ),
            (
                "src/inner.rs",
                "use crate::foo::Top;\npub fn t() -> Top { Top }\n",
            ),
        ],
    )
}

#[test_case(&["use", "lib::lib"], "crate::foo::Bar\n"; "use by its name")]
#[test_case(&["use", "pkg::lib"], "crate::foo::Bar\n"; "use led by crate name")]
#[test_case(&["use", "lib::lib::inner"], "crate::foo::Baz\n"; "use child by its name")]
#[test_case(&["use", "pkg::lib::inner"], "crate::foo::Baz\n"; "use child led by crate name")]
#[test_case(&["use", "lib::inner"], "crate::foo::Top\n"; "use top level namesake led by lib")]
#[test_case(&["use", "inner"], "crate::foo::Top\n"; "use top level namesake")]
#[test_case(
    &["use", "other"],
    "crate::Reexported\ncrate::lib::g\ncrate::lib::inner::*\n";
    "use top level namesake in the root file"
)]
#[test_case(&["why", "lib::lib", "foo"], "crate::foo::Bar\n"; "why source")]
#[test_case(&["why", "lib::lib::inner", "foo"], "crate::foo::Baz\n"; "why child source")]
#[test_case(&["why", "other", "lib::lib"], "crate::lib::g\n"; "why target")]
#[test_case(&["why", "other", "lib"], "crate::Reexported\n"; "why target root")]
#[test_case(&["why", "other", "lib::lib::inner"], "crate::lib::inner::*\n"; "why child target")]
#[test_case(&["deps", "--path", "other", "lib::lib::inner"], "other -> lib::lib::inner\n"; "deps path")]
#[test_case(&["list", "lib::lib"], "lib::lib\nlib::lib::inner\nlib::lib::other\n"; "list")]
#[test_case(
    &["use", "-G", "other"],
    "crate::Reexported\ncrate::lib::g\ncrate::lib::inner::f\n";
    "glob"
)]
fn an_inline_module_named_lib_is_reachable(args: &[&str], expected: &str) {
    let krate = inline_module_named_lib();
    assert_eq!(crawk_stdout(krate.path(), 0, args), expected);
}

#[test_case(&[], &["foo", "inner", "lib", "lib::lib", "lib::lib::inner", "lib::lib::other", "other"]; "all")]
#[test_case(&["-d", "1"], &["foo", "inner", "lib", "lib::lib", "other"]; "depth 1")]
fn an_inline_module_named_lib_is_listed_apart_from_the_root(flags: &[&str], expected: &[&str]) {
    let krate = inline_module_named_lib();
    let args = [&["list"], flags].concat();
    assert_eq!(lines(&crawk_stdout(krate.path(), 0, &args)), expected);
}

#[test]
fn deps_keeps_an_inline_module_named_lib_apart_from_the_root() {
    let krate = inline_module_named_lib();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-a"]);
    for edge in [
        "lib::lib -> foo [Bar]",
        "lib::lib::inner -> foo [Baz]",
        "other -> lib [Reexported]",
        "other -> lib::lib [g]",
        "other -> lib::lib::inner",
    ] {
        assert!(
            lines(&stdout).contains(&edge),
            "{edge} missing in:\n{stdout}"
        );
    }
}

#[test]
fn deps_depth_counts_an_inline_module_named_lib_as_top_level() {
    let krate = inline_module_named_lib();
    let stdout = crawk_stdout(krate.path(), 0, &["deps", "-d", "1"]);
    assert!(lines(&stdout).contains(&"other -> lib::lib"), "{stdout}");
    assert!(!stdout.contains("lib::lib::inner"), "{stdout}");
}

#[test]
fn check_tells_an_inline_module_named_lib_from_the_root() {
    let krate = inline_module_named_lib();
    write_rules(
        krate.path(),
        "[check]\n[[check.deny]]\nfrom = \"other\"\nto = \"lib::lib\"\n",
    );
    let output = crawk_output(krate.path(), &["check", "-a"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stdout}");
    assert!(stdout.contains("other -> lib::lib [g]"), "{stdout}");
    assert!(!stdout.contains("Reexported"), "{stdout}");
}

#[test]
fn check_init_gives_an_inline_module_named_lib_its_own_layer_entry() {
    let krate = inline_module_named_lib();
    crawk_stdout(krate.path(), 0, &["check", "--init"]);
    let config = fs::read_to_string(krate.path().join("crawk.toml")).unwrap();
    assert!(config.contains("\"lib::lib\""), "{config}");
}
