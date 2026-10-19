use crate::common::crawk_hyphenated;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// hyphenated package (`hyphenated-pkg`, library crate `hyphenated_pkg`) — list command
// ============================================================================

// The binary target keeps its cargo name, hyphen and all: that is the name of
// the executable, not a crate name.
#[test]
fn should_list_hyphenated() {
    assert_cmd_snapshot!(crawk_hyphenated().arg("list"));
}

// MODULE_PATH may lead with either spelling of the library's name; the root
// entry echoes the one that was asked for.
#[test_case("hyphenated_pkg", "crate_name" ; "crate name")]
#[test_case("hyphenated-pkg", "package_name" ; "package name")]
#[test_case("hyphenated_pkg::render", "crate_name_and_module" ; "crate name and module")]
#[test_case("hyphenated-pkg::render", "package_name_and_module" ; "package name and module")]
fn should_list_path_led_by_library_name_in_hyphenated(module: &str, case: &str) {
    assert_cmd_snapshot!(
        format!("hyphenated_list_{case}"),
        crawk_hyphenated().args(["list", module])
    );
}

// The integration test's root is listed under its file stem; that name, alone
// or leading a path, must address the test target.
#[test_case("smoke", "root" ; "root")]
#[test_case("smoke::support", "root_and_module" ; "root and module")]
fn should_list_path_led_by_test_root_in_hyphenated(module: &str, case: &str) {
    assert_cmd_snapshot!(
        format!("hyphenated_list_test_{case}"),
        crawk_hyphenated().args(["list", module, "-t"])
    );
}
