use crate::common::{backtrace_filters, crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Error scenarios
// ============================================================================

#[test]
fn should_error_on_nonexistent_source() {
    let mut settings = insta::Settings::clone_current();
    for (pattern, replacement) in backtrace_filters() {
        settings.add_filter(pattern, replacement);
    }
    settings.bind(|| {
        assert_cmd_snapshot!(crawk_modules().args(["why", "nonexistent", "file_module"]), @r"
        success: false
        exit_code: 1
        ----- stdout -----

        ----- stderr -----
        Error: Module 'nonexistent' not found
        ");
    });
}

#[test]
fn should_error_on_nonexistent_source_own_crate() {
    let mut settings = insta::Settings::clone_current();
    for (pattern, replacement) in backtrace_filters() {
        settings.add_filter(pattern, replacement);
    }
    settings.bind(|| {
        assert_cmd_snapshot!(crawk().args(["why", "nonexistent", "reference"]));
    });
}

#[test]
fn should_error_on_invalid_module_path() {
    assert_cmd_snapshot!(crawk().args(["why", "invalid..path", "reference"]), @"
    success: false
    exit_code: 2
    ----- stdout -----

    ----- stderr -----
    error: invalid value 'invalid..path' for '<SOURCE>': module path 'invalid..path' contains invalid characters (only a-z, A-Z, 0-9, _, - and :: are allowed)

    For more information, try '--help'.
    ");
}
