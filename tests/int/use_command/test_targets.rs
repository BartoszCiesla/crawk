use crate::common::{backtrace_filters, crawk, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// === Own crate (crawk's own integration test target) ===

#[test]
fn should_analyze_test_module_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("use").arg("int::command").arg("-t"));
}

#[test]
fn should_analyze_test_module_subtree_recursive() {
    assert_cmd_snapshot!(
        crawk()
            .arg("use")
            .arg("int::list_command")
            .arg("-t")
            .arg("-r")
    );
}

#[test]
fn should_analyze_test_module_with_no_deps() {
    assert_cmd_snapshot!(crawk().arg("use").arg("common").arg("-t"));
}

#[test]
fn should_analyze_leaf_test_module_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("use").arg("int::version").arg("-t"));
}

// === Fixture crate (fixtures/modules) ===

#[test]
fn should_analyze_test_module_for_fixture() {
    assert_cmd_snapshot!(crawk_modules().arg("use").arg("helpers").arg("-t"));
}

// === Without -t flag: must fail ===

#[test]
fn should_error_for_test_module_without_include_tests_flag() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk().arg("use").arg("int::command"));
    });
}

#[test]
fn should_error_for_test_module_without_include_tests_flag_fixture() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk_modules().arg("use").arg("helpers"));
    });
}

// === Nonexistent module: must fail even with -t ===

#[test]
fn should_error_for_nonexistent_test_module_with_include_tests() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(crawk().arg("use").arg("nonexistent_test_mod").arg("-t"));
    });
}

// === Flags combined with test target modules ===

#[test]
fn should_analyze_test_module_with_expand() {
    assert_cmd_snapshot!(
        crawk()
            .arg("use")
            .arg("int::list_command::test_targets")
            .arg("-t")
            .arg("-e")
    );
}

#[test]
fn should_analyze_test_module_with_grouped_format() {
    assert_cmd_snapshot!(
        crawk()
            .arg("use")
            .arg("int::list_command")
            .arg("-t")
            .arg("-r")
            .arg("--format")
            .arg("grouped")
    );
}

#[test]
fn should_analyze_test_module_with_depth() {
    assert_cmd_snapshot!(
        crawk()
            .arg("use")
            .arg("int")
            .arg("-t")
            .arg("-r")
            .arg("-d")
            .arg("1")
    );
}
