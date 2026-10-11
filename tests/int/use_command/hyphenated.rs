use crate::common::crawk_hyphenated;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// hyphenated package (`hyphenated-pkg`, library crate `hyphenated_pkg`) — use command
// ============================================================================

// The binary reaches the library by its crate name; the package name, hyphen
// and all, never appears in source.
#[test]
fn should_use_main_in_hyphenated() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["use", "main"]));
}

// The glob is written behind the crate name, so resolving it means finding the
// library under that name.
#[test]
fn should_use_main_with_resolved_globs_in_hyphenated() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["use", "main", "-G"]));
}

// A module of an integration test reaches the library the same way.
#[test]
fn should_use_test_module_in_hyphenated() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["use", "support", "-t"]));
}

// MODULE_PATH may lead with either spelling of the library's name: the crate
// name Rust source uses, or the package name of `Cargo.toml`.
#[test_case("hyphenated_pkg", "crate_name" ; "crate name")]
#[test_case("hyphenated-pkg", "package_name" ; "package name")]
#[test_case("hyphenated_pkg::render", "crate_name_and_module" ; "crate name and module")]
#[test_case("hyphenated-pkg::render", "package_name_and_module" ; "package name and module")]
fn should_use_path_led_by_library_name_in_hyphenated(module: &str, case: &str) {
    assert_cmd_snapshot!(
        format!("hyphenated_use_{case}"),
        crawk_hyphenated().args(["use", module])
    );
}
