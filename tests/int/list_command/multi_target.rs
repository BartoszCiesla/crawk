use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

#[test]
fn should_list_all_targets_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list"));
}

#[test]
fn should_list_all_targets_with_tests() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-t"));
}

#[test]
fn should_list_single_target_no_prefix() {
    // When a module_path is given, no prefix should appear
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("nesting"));
}

#[test]
fn should_list_table_multi_target() {
    assert_cmd_snapshot!(crawk().arg("list").arg("--format").arg("table"));
}

#[test]
fn should_list_table_multi_target_with_tests() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("-t")
            .arg("--format")
            .arg("table")
    );
}

#[test]
fn should_list_multi_target_with_source() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-t").arg("-s"));
}

#[test]
fn should_list_multi_target_with_depth() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-t").arg("-d").arg("1"));
}

#[test]
fn should_list_multi_target_with_filter() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("-t")
            .arg("-F")
            .arg("helper")
    );
}

#[test]
fn should_list_multi_target_with_visibility() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-t").arg("-V"));
}
