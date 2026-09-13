use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// The allowance misses a real edge: cli -> web::repo is outside
// `to = ["analyzer"]` → one RESTRICT row citing the full allowance (exit 1).
#[test]
fn should_report_target_outside_allowance() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_violated.toml")
    );
}

// `--show-apis` annotates the offending edge with the symbols that create it.
#[test]
fn should_annotate_restricted_edge_with_apis() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_violated.toml")
            .arg("--show-apis")
    );
}

// An allowance covering every outbound edge of `cli`: no output, exit 0.
#[test]
fn should_pass_when_allowance_covers_all_edges() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_clean.toml")
    );
}

// `to = []` closes `web` off from the rest of the crate; every real web edge
// stays inside the `from` scope (siblings included), so the run is clean.
#[test]
fn should_exempt_edges_inside_the_scope() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_subtree.toml")
    );
}

// Overlapping rules intersect their allowances: the broad rule is satisfied,
// the narrow one fires → exactly one RESTRICT row.
#[test]
fn should_intersect_overlapping_allowances() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_intersect.toml")
    );
}

// Deny carves a hole in the restrict allowance: the edge passes restrict but
// breaks the deny rule → one DENY row, zero RESTRICT rows.
#[test]
fn should_compose_with_deny() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_restrict_with_deny.toml")
    );
}
