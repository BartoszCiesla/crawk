use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

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
