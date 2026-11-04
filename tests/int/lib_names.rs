//! The library's root goes by `lib`, its modules by their bare path, and a root
//! alias (`lib`, the crate name, the package name) may lead a path typed on the
//! command line.
//!
//! A library module named like an alias stays reachable: the name crawk prints
//! for it reads back as that module (`my_crate::inner`), so does the spelling led
//! by an alias (`lib::my_crate::inner`), and `lib` alone is the root. The crates
//! are written into tempdirs: a tempdir path would leak into the header of an
//! `insta` snapshot, so the assertions read the output directly.

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::{lines, scratch_crate};

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
/// alias. Its children `inner` and `other` have top-level namesakes.
fn inline_module_named_lib() -> TempDir {
    scratch_crate(
        "pkg",
        &[
            (
                "src/lib.rs",
                "pub mod foo;\npub mod inner;\n\
                 pub mod lib {\n    use crate::foo::Bar;\n    pub fn g() -> Bar { Bar }\n    \
                 pub mod inner {\n        use crate::foo::Baz;\n        pub fn f() -> Baz { Baz }\n    }\n    \
                 pub mod other {}\n}\n\
                 pub mod other {\n    use crate::lib::inner::*;\n    pub fn k() { f(); }\n}\n",
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

#[test_case(&["use", "lib::lib"], "crate::foo::Bar\n"; "use led by lib")]
#[test_case(&["use", "lib::lib::inner"], "crate::foo::Baz\n"; "use child led by lib")]
#[test_case(&["use", "lib::inner"], "crate::foo::Baz\n"; "use child by its name")]
#[test_case(&["use", "inner"], "crate::foo::Top\n"; "use top level namesake")]
#[test_case(&["use", "pkg::inner"], "crate::foo::Top\n"; "use top level namesake led by crate name")]
#[test_case(&["use", "other"], "crate::lib::inner::*\n"; "use top level namesake in the root file")]
#[test_case(&["why", "lib::lib", "foo"], "crate::foo::Bar\n"; "why source")]
#[test_case(&["why", "lib::inner", "foo"], "crate::foo::Baz\n"; "why child source")]
#[test_case(&["why", "other", "lib::inner"], "crate::lib::inner::*\n"; "why target")]
#[test_case(&["deps", "--path", "other", "lib::inner"], "other -> lib::inner\n"; "deps path")]
#[test_case(&["list", "lib::lib"], "lib\nlib::inner\nlib::other\n"; "list")]
#[test_case(&["use", "-G", "other"], "crate::lib::inner::f\n"; "glob")]
fn an_inline_module_named_lib_is_reachable(args: &[&str], expected: &str) {
    let krate = inline_module_named_lib();
    assert_eq!(crawk_stdout(krate.path(), 0, args), expected);
}
