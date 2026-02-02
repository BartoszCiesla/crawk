use crate::common::command::TestArgs;
use crate::common::test::CmdTestCase;
use assert_cmd::assert::Assert;
use predicates::str::diff;
use std::convert::Into;

pub(crate) struct TestHelpCmd {
    help_command: Vec<String>,
    expected_output: String,
    failure: bool,
}

impl TestHelpCmd {
    pub(crate) fn new(command: Vec<impl Into<String>>, expected_output: String) -> Self {
        let help_command = command.into_iter().map(Into::into).collect();
        Self {
            help_command,
            expected_output,
            failure: false,
        }
    }

    pub(crate) fn failure(command: Vec<impl Into<String>>, expected_output: String) -> Self {
        let help_command = command.into_iter().map(Into::into).collect();
        Self {
            help_command,
            expected_output,
            failure: true,
        }
    }
}

impl CmdTestCase for TestHelpCmd {
    fn get_command(&self) -> TestArgs {
        TestArgs::new().args(self.help_command.clone())
    }

    fn verify_command(&self, command_state: Assert) {
        if self.failure {
            command_state
                .failure()
                .stderr(diff(self.expected_output.clone()));
        } else {
            command_state
                .success()
                .stdout(diff(self.expected_output.clone()));
        }
    }
}
