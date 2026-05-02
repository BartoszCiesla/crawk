use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

#[test]
fn should_list_modules_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("list"));
}

#[test]
fn should_list_modules_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list"));
}

#[test]
fn should_list_subtree_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("nesting"));
}

#[test]
fn should_list_subtree_for_dir_module() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("dir_module"));
}

#[test]
fn should_list_with_root_for_lib() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("lib"));
}

#[test]
fn should_list_with_root_for_own_lib() {
    assert_cmd_snapshot!(crawk().arg("list").arg("lib"));
}

#[test_case("crawk" ; "crate name")]
#[test_case("main" ; "binary stem")]
fn should_list_with_root_for_target(target: &str) {
    assert_cmd_snapshot!(target, crawk().arg("list").arg(target));
}
