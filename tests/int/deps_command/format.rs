use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// --format dot
// ============================================================================

#[test]
fn should_deps_dot_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("deps").arg("-f").arg("dot"));
}

#[test]
fn should_deps_dot_depth_1_for_fixture_crate() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("deps")
            .arg("-f")
            .arg("dot")
            .arg("-d")
            .arg("1")
    );
}

// ============================================================================
// --format grouped
// ============================================================================

#[test]
fn should_deps_grouped_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("-f").arg("grouped"));
}

#[test]
fn should_deps_grouped_for_fixture_crate() {
    assert_cmd_snapshot!(crawk_modules().arg("deps").arg("-f").arg("grouped"));
}

#[test]
fn should_deps_grouped_depth_1_for_own_crate() {
    assert_cmd_snapshot!(
        crawk()
            .arg("deps")
            .arg("-f")
            .arg("grouped")
            .arg("-d")
            .arg("1")
    );
}

#[test]
fn should_deps_grouped_depth_1_for_fixture_crate() {
    assert_cmd_snapshot!(
        crawk_modules()
            .arg("deps")
            .arg("-f")
            .arg("grouped")
            .arg("-d")
            .arg("1")
    );
}
