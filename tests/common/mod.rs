use insta_cmd::get_cargo_bin;
use std::process::Command;

pub(crate) fn crawk() -> Command {
    Command::new(get_cargo_bin("crawk"))
}
