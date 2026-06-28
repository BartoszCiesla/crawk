use crate::common::crawk_check;
use insta_cmd::assert_cmd_snapshot;

// The fixture's `.crawk.toml` defines two independent layer groups; every edge is
// downward within its group (cli -> web::repo is cross-group, unconstrained), so
// the clean run produces no output and exit code 0.
#[test]
fn should_pass_clean_fixture() {
    assert_cmd_snapshot!(crawk_check());
}
