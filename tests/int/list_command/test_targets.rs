use crate::common::{backtrace_filters, crawk, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// === Own crate (crawk's own integration test target) ===

#[test]
fn should_list_test_module_subtree_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list").arg("int").arg("-t"));
}

#[test]
fn should_list_nested_test_module_subtree_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list").arg("int::list_command").arg("-t"));
}

#[test]
fn should_list_leaf_test_module_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list").arg("int::version").arg("-t"));
}

#[test]
fn should_list_common_test_module_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("list").arg("common").arg("-t"));
}

// === Fixture crate (fixtures/modules) ===

#[test]
fn should_list_test_module_subtree_for_fixture() {
    assert_cmd_snapshot!(crawk_modules().arg("list").arg("helpers").arg("-t"));
}

// === Without -t flag: must fail ===

#[test]
fn should_error_for_test_module_without_include_tests_flag() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk().arg("list").arg("int"));
    });
}

#[test]
fn should_error_for_test_module_without_include_tests_flag_fixture() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk_modules().arg("list").arg("helpers"));
    });
}

// === Nonexistent module within test target: must fail even with -t ===

#[test]
fn should_error_for_nonexistent_test_module_with_include_tests() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk().arg("list").arg("nonexistent_test_mod").arg("-t"));
    });
}

// === Flags combined with test target subtree ===

#[test]
fn should_list_test_module_subtree_with_source() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("helpers")
            .arg("-t")
            .arg("-s")
    );
}

#[test]
fn should_list_test_module_subtree_with_depth() {
    assert_cmd_snapshot!(crawk().arg("list").arg("int").arg("-t").arg("-d").arg("2"));
}

#[test]
fn should_list_test_module_subtree_with_filter() {
    assert_cmd_snapshot!(
        crawk()
            .arg("list")
            .arg("int")
            .arg("-t")
            .arg("-F")
            .arg("command")
    );
}

#[test]
fn should_list_test_module_subtree_with_table_format() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("list")
            .arg("helpers")
            .arg("-t")
            .arg("--format")
            .arg("table")
    );
}
