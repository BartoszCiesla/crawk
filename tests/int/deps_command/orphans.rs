use crate::common::{crawk, crawk_cycles, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Orphans — cycles fixture (standalone has no incoming edges)
// ============================================================================

#[test]
fn should_orphans_for_cycles_fixture() {
    assert_cmd_snapshot!(crawk_cycles().arg("deps").arg("--orphans"));
}

#[test]
fn should_orphans_with_depth_1_for_cycles_fixture() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--orphans")
            .arg("-d")
            .arg("1")
    );
}

// ============================================================================
// Orphans — modules fixture
// ============================================================================

#[test]
fn should_orphans_for_modules_fixture() {
    assert_cmd_snapshot!(crawk_modules().arg("deps").arg("--orphans"));
}

// ============================================================================
// Orphans — own crate
// ============================================================================

#[test]
fn should_orphans_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("--orphans"));
}

// ============================================================================
// Orphans with --include-tests
// ============================================================================

#[test]
fn should_orphans_with_include_tests_for_cycles_fixture() {
    assert_cmd_snapshot!(crawk_cycles().arg("deps").arg("--orphans").arg("-t"));
}

// ============================================================================
// Mutual exclusivity with --cycles
// ============================================================================

#[test]
fn should_error_orphans_with_cycles() {
    assert_cmd_snapshot!(crawk_cycles().arg("deps").arg("--orphans").arg("--cycles"));
}
