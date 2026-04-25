use crate::common::crawk_modules;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

#[test]
fn should_list_with_source() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-s"));
}

#[test]
fn should_list_with_include_tests() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-t"));
}

#[test_case("1"; "depth 1")]
#[test_case("2"; "depth 2")]
fn should_list_with_depth(depth: &str) {
    let snapshot_name = format!("depth_{depth}");
    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("list").arg("-d").arg(depth)
    );
}

#[test]
fn should_list_with_table_format() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("--format").arg("table"));
}

#[test]
fn should_list_with_table_format_and_source() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("--format")
            .arg("table")
            .arg("-s")
    );
}

#[test]
fn should_list_subtree_with_source() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("nesting").arg("-s"));
}

#[test]
fn should_list_subtree_with_depth() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("glob_showcase")
            .arg("-d")
            .arg("2")
    );
}

#[test]
fn should_list_with_visibility() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-V"));
}

#[test]
fn should_list_with_visibility_and_source() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("-V").arg("-s"));
}

#[test]
fn should_list_with_table_format_and_visibility() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("--format")
            .arg("table")
            .arg("-V")
    );
}

#[test]
fn should_list_subtree_with_visibility() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("visibility").arg("-V"));
}
