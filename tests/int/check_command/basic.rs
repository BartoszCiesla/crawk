use crate::common::{crawk_check, crawk_cycles};
use insta_cmd::assert_cmd_snapshot;

// The fixture's `.crawk.toml` defines two independent layer groups; every edge is
// downward within its group (cli -> web::repo is cross-group, unconstrained), so
// the clean run produces no output and exit code 0.
#[test]
fn should_pass_clean_fixture() {
    assert_cmd_snapshot!(crawk_check());
}

// One config on the cycles fixture triggers all four rule kinds at once: the
// edge alpha -> beta breaks deny, an empty restrict allowance, and an inverted
// layer order, and the alpha/beta/gamma loop adds the CYCLE rows. Pins the
// report order (DENY, RESTRICT, LAYER, CYCLE) and the kind-column alignment of
// a mixed report — the only snapshot where the padding is non-trivial.
#[test]
fn should_report_all_kinds_in_one_run() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("check")
            .arg("-c")
            .arg("fixtures/cycles/rules_all_kinds.toml")
    );
}
