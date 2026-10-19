use crate::common::crawk_hyphenated;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// hyphenated package (`hyphenated-pkg`, library crate `hyphenated_pkg`) — deps command
// ============================================================================

// The binary's references are written behind the crate name: each must land on
// the library module it names, the crate-root re-export on the synthetic `lib`.
#[test]
fn should_deps_hyphenated() {
    assert_cmd_snapshot!(crawk_hyphenated().arg("deps"));
}

// The crate name is stripped before the API symbols are read off the path.
#[test]
fn should_deps_hyphenated_with_apis() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["deps", "-a"]));
}

// The integration test is analysed from its own root, `tests/smoke.rs`: the
// submodule it declares reaches the library through the crate name.
#[test]
fn should_deps_hyphenated_with_include_tests() {
    assert_cmd_snapshot!(crawk_hyphenated().args(["deps", "-t"]));
}
