//! A target root (`lib`, `bin@<name>`, `test@<name>`) owns only the items of
//! its entry file that sit outside every inline `mod x { ... }`. Each inline
//! module there is a child analysed under its own name, so its references are
//! never the root's as well: not in `use`, not in `why`, no `deps` edge from the
//! root. The crates are written into tempdirs: a tempdir path would leak into
//! the header of an `insta` snapshot, so the assertions read the output directly.

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::scratch_crate;

/// Package `rootc` whose three roots each reach `top` (or the binary's `deep`)
/// themselves and hold an inline module that reaches something else: `a::b` in
/// the library and the test both reach `bar`, the binary's `deep` reaches its
/// `helper`.
fn roots_with_inline_modules() -> TempDir {
    scratch_crate(
        "rootc",
        &[
            (
                "src/lib.rs",
                "pub mod bar;\npub mod top;\n\
                 pub fn root() -> crate::top::Top { crate::top::Top }\n\
                 pub mod a { pub mod b { use crate::bar::Bar; pub fn f() -> Bar { Bar } } }\n",
            ),
            ("src/bar.rs", "pub struct Bar;\n"),
            ("src/top.rs", "pub struct Top;\n"),
            (
                "src/bin/tool.rs",
                "mod helper { pub struct H; }\n\
                 mod deep { pub fn g() -> crate::helper::H { crate::helper::H } }\n\
                 fn main() { crate::deep::g(); }\n",
            ),
            (
                "tests/it.rs",
                "mod a { pub mod b { use rootc::bar::Bar; pub fn f() -> Bar { Bar } } }\n\
                 fn root() -> rootc::top::Top { rootc::top::Top }\n",
            ),
        ],
    )
}

#[test_case(&["use", "lib"], "crate::top::Top\n"; "use on the library root")]
#[test_case(
    &["use", "lib", "-r", "-f", "grouped"],
    "lib\n - crate::top::Top\na\na::b\n - crate::bar::Bar\nbar\ntop\n";
    "use recursive keeps each inline module under its own name"
)]
#[test_case(&["why", "lib", "bar"], ""; "why the library root")]
#[test_case(&["why", "lib", "top"], "crate::top::Top\n"; "why the library root, own reference")]
#[test_case(&["use", "bin@tool"], "crate::deep::g\n"; "use on a binary root")]
#[test_case(&["why", "bin@tool", "bin@tool::helper"], ""; "why a binary root")]
#[test_case(
    &["why", "bin@tool::deep", "bin@tool::helper"],
    "crate::helper::H\n";
    "why the binary's inline module"
)]
#[test_case(&["use", "test@it", "-t"], "rootc::top::Top\n"; "use on a test root")]
#[test_case(&["why", "-t", "test@it", "bar"], ""; "why a test root")]
#[test_case(
    &["why", "-t", "test@it::a::b", "bar"],
    "rootc::bar::Bar\n";
    "why the test's inline module"
)]
#[test_case(
    &["deps"],
    "a::b -> bar\n\
     bin@tool -> bin@tool::deep\n\
     bin@tool::deep -> bin@tool::helper\n\
     lib -> top\n";
    "deps"
)]
#[test_case(
    &["deps", "-t"],
    "a::b -> bar\n\
     bin@tool -> bin@tool::deep\n\
     bin@tool::deep -> bin@tool::helper\n\
     lib -> top\n\
     test@it -> top\n\
     test@it::a::b -> bar\n";
    "deps with tests"
)]
fn a_target_root_does_not_take_its_inline_modules_references(args: &[&str], expected: &str) {
    let krate = roots_with_inline_modules();
    assert_eq!(crawk_stdout(krate.path(), 0, args), expected);
}
