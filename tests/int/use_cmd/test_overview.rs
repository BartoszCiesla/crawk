use crate::common::help::TestHelpCmd;
use crate::common::test::CmdTestCase;
use test_log::test;

#[test]
fn should_overview_match() {
    let mut test_case = TestHelpCmd::failure(
        vec!["use"],
        r"error: the following required arguments were not provided:
  <MODULE_PATH>

Usage: crawk use <MODULE_PATH>

For more information, try '--help'.
"
        .into(),
    );
    test_case.execute();
}
