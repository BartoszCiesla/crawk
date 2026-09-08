use std::process::Command;

use crate::common::{crawk_check, crawk_cycles};
use insta_cmd::assert_cmd_snapshot;

/// `crawk -p fixtures/cycles check -c <config>`.
///
/// `crawk_cycles()` stops before the subcommand (it also serves `deps`), unlike
/// `crawk_check()`, so the subcommand is appended here.
fn check_cycles(config: &str) -> Command {
    let mut cmd = crawk_cycles();
    cmd.arg("check")
        .arg("-c")
        .arg(format!("fixtures/cycles/{config}"));
    cmd
}

// The alpha -> beta -> gamma loop yields one CYCLE row per edge, each citing the
// whole loop. The nest <-> nest::inner loop is absent: containment is skipped by
// default.
#[test]
fn should_report_cycle_when_enabled() {
    assert_cmd_snapshot!(check_cycles("rules_deny_cycles.toml"));
}

// `--show-apis` annotates each cycle edge with the symbols that create it.
#[test]
fn should_annotate_cycle_with_apis() {
    assert_cmd_snapshot!(check_cycles("rules_deny_cycles.toml").arg("--show-apis"));
}

// Without `deny-cycles` the loop is invisible to `check`, even though `deps
// --cycles` reports it — the flag is what turns the rule on.
#[test]
fn should_ignore_cycle_when_disabled() {
    assert_cmd_snapshot!(check_cycles("rules_no_cycle_check.toml"));
}

// A grandfathered loop passes. Both loops are gone from the report: alpha's via
// the allowlist, nest's via the structural filter.
#[test]
fn should_pass_when_cycle_is_allowlisted() {
    assert_cmd_snapshot!(check_cycles("rules_allow_cycle.toml"));
}

// Subset matching: an entry that does not name every module of the loop does not
// cover it. The entry also warns as stale, since it matched nothing.
#[test]
fn should_report_when_allowlist_misses_a_module() {
    assert_cmd_snapshot!(check_cycles("rules_allow_cycle_partial.toml"));
}

// An entry that matches no cycle (the loop was untangled) warns on stderr but
// leaves the exit code alone — fixing a cycle must not break the build.
#[test]
fn should_warn_on_stale_allowlist_entry() {
    assert_cmd_snapshot!(check_cycles("rules_allow_cycle_stale.toml"));
}

// An allowlist without `deny-cycles` is inert config; loading it says so.
#[test]
fn should_warn_when_allowlist_is_inert() {
    assert_cmd_snapshot!(check_cycles("rules_allow_cycle_inert.toml"));
}

// `deny-parent-child-cycles` turns the structural filter off, so the
// nest <-> nest::inner containment loop is reported as well.
#[test]
fn should_report_parent_child_cycle_when_knob_set() {
    assert_cmd_snapshot!(check_cycles("rules_parent_child.toml"));
}

// The rule is on and the crate has no loops: clean, no output.
#[test]
fn should_pass_acyclic_crate_with_rule_enabled() {
    assert_cmd_snapshot!(
        crawk_check()
            .arg("-c")
            .arg("fixtures/check/rules_deny_cycles.toml")
    );
}
