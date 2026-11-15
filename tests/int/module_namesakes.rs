//! A lone name spelled like a child module — a local variable, a function, a
//! macro — is a value or a macro, never the module, so it is no reference: no
//! `use` line, no `deps` edge, no `why` answer, no `check` violation. A
//! single-segment import of the module (`use child as c;`) does name it and
//! counts. The crates are written into tempdirs: a tempdir path would leak into
//! the header of an `insta` snapshot, so the assertions read the output directly.

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk_stdout;
use crate::int::bin_names::{scratch_crate, write_rules};

/// Package `my-crate` whose root has children `child` and `format`, a local and
/// a function both named `child`, and a `format!` call — none of them reaching
/// a module — beside `a`, which imports its own `child` as `c`.
fn namesakes() -> TempDir {
    scratch_crate(
        "my-crate",
        &[
            (
                "src/lib.rs",
                "pub mod a;\npub mod child;\npub mod format;\n\n\
                 pub fn count() -> usize {\n    let child = vec![1, 2];\n    child.len()\n}\n\n\
                 pub fn child() -> u8 {\n    1\n}\n\n\
                 pub fn label() -> String {\n    format!(\"{}\", child())\n}\n",
            ),
            ("src/child.rs", "pub fn f() {}\n"),
            ("src/format.rs", "pub fn g() {}\n"),
            (
                "src/a.rs",
                "pub mod child;\nuse child as c;\npub fn h() {\n    c::f();\n}\n",
            ),
            ("src/a/child.rs", "pub fn f() {}\n"),
        ],
    )
}

#[test_case(&["use", "lib"], ""; "use on the root")]
#[test_case(&["use", "a"], "crate::a::child as c\n"; "use on the importing module")]
#[test_case(&["deps"], "a -> a::child\n"; "deps")]
#[test_case(&["why", "lib", "child"], ""; "why through a local and a function")]
#[test_case(&["why", "lib", "format"], ""; "why through a macro")]
fn a_lone_name_spelled_like_a_child_module_is_no_reference(args: &[&str], expected: &str) {
    let krate = namesakes();
    assert_eq!(crawk_stdout(krate.path(), 0, args), expected);
}

#[test]
fn check_passes_a_ban_on_a_child_reached_only_by_namesakes() {
    let krate = namesakes();
    write_rules(
        krate.path(),
        "[check]\n\
         [[check.deny]]\nfrom = \"lib\"\nto = \"child\"\n\
         [[check.deny]]\nfrom = \"lib\"\nto = \"format\"\n",
    );
    assert_eq!(crawk_stdout(krate.path(), 0, &["check"]), "");
}
