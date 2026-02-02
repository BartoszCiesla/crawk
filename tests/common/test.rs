use crate::common::command::TestArgs;
use assert_cmd::assert::{Assert, OutputAssertExt};
use std::process::Command;
use tracing::info;

pub(crate) trait CmdTestCase {
    fn setup(&mut self) {
        info!("Empty setup for the test case");
    }

    fn get_command(&self) -> TestArgs;

    fn execute(&mut self) {
        self.setup();
        info!("Executing the test case");

        // Get the command
        let mut command = Command::new(assert_cmd::cargo::cargo_bin!("crawk"));

        // Get test case arguments, options and environment variables
        let test_args = self.get_command();
        command.envs(test_args.get_env());
        command.args(test_args.get_opts_and_args());

        // Print used environment variables and command with all arguments.
        info!(
            "Running: {} {} {}",
            command
                .get_envs()
                .map(|k| format!(
                    "{}={}",
                    k.0.to_str().unwrap(),
                    k.1.unwrap().to_str().unwrap()
                ))
                .collect::<Vec<String>>()
                .join(" "),
            command.get_program().to_str().unwrap(),
            test_args.get_opts_and_args().join(" ")
        );

        self.verify_command(command.assert());
        info!("End of the test case");
        self.teardown();
    }

    fn verify_command(&self, command_state: Assert);

    fn teardown(&self) {
        info!("Empty teardown for the test case");
    }
}
