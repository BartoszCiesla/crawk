use insta_cmd::get_cargo_bin;
use std::process::Command;

pub(crate) fn crawk() -> Command {
    Command::new(get_cargo_bin("crawk"))
}

pub(crate) fn crawk_modules() -> Command {
    let mut cmd = crawk();
    cmd.arg("-p")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/modules"));
    cmd
}
