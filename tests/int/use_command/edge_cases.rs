use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Edge cases for use command
// ============================================================================

#[test]
fn should_handle_grouped_without_recursive() {
    assert_cmd_snapshot!(crawk().arg("use").arg("lib").arg("--format").arg("grouped"));
}

#[test]
fn should_handle_resolve_globs_without_expand() {
    assert_cmd_snapshot!(crawk().arg("use").arg("lib").arg("-G"));
}

#[test]
fn should_handle_depth_without_expand() {
    assert_cmd_snapshot!(crawk().arg("use").arg("lib").arg("-d").arg("1"));
}

#[test]
fn should_handle_all_flags_combined() {
    assert_cmd_snapshot!(
        crawk()
            .arg("use")
            .arg("lib")
            .arg("-r")
            .arg("-t")
            .arg("-e")
            .arg("--format")
            .arg("grouped")
            .arg("-G")
            .arg("-d")
            .arg("2")
    );
}
