use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;

#[test]
fn should_filter_by_substring() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-F").arg("glob"));
}

#[test]
fn should_filter_by_substring_with_source() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("-F")
            .arg("nesting")
            .arg("-s")
    );
}

#[test]
fn should_filter_with_no_matches() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("-F")
            .arg("nonexistent_module_xyz")
    );
}

#[test]
fn should_filter_combined_with_depth() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("-F")
            .arg("glob")
            .arg("-d")
            .arg("1")
    );
}
