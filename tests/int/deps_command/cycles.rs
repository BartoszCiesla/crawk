use crate::common::{crawk, crawk_cycles, crawk_modules};
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Cycles — fixture crate (has alpha->beta->gamma->alpha cycle)
// ============================================================================

#[test]
fn should_cycles_plain_for_cycles_fixture() {
    assert_cmd_snapshot!(crawk_cycles().arg("deps").arg("--cycles"));
}

#[test]
fn should_cycles_grouped_for_cycles_fixture() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--cycles")
            .arg("-f")
            .arg("grouped")
    );
}

#[test]
fn should_cycles_dot_for_cycles_fixture() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--cycles")
            .arg("-f")
            .arg("dot")
    );
}

#[test]
fn should_cycles_highlight_dot_for_cycles_fixture() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--cycles")
            .arg("highlight")
            .arg("-f")
            .arg("dot")
    );
}

#[test]
fn should_cycles_with_show_apis_for_cycles_fixture() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--cycles")
            .arg("--show-apis")
    );
}

// ============================================================================
// Cycles present in modules fixture
// ============================================================================

#[test]
fn should_cycles_for_modules_fixture() {
    assert_cmd_snapshot!(crawk_modules().arg("deps").arg("--cycles"));
}

// ============================================================================
// No cycles — own crate (DAG)
// ============================================================================

#[test]
fn should_no_cycles_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("--cycles"));
}

// ============================================================================
// Highlight warning on non-DOT format
// ============================================================================

#[test]
fn should_cycles_highlight_warning_plain() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--cycles")
            .arg("highlight")
            .arg("-f")
            .arg("plain")
    );
}
