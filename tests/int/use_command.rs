use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;
use test_case::{test_case, test_matrix};

#[test]
fn should_use_command_overview_match() {
    assert_cmd_snapshot!(crawk().arg("use"));
}

#[test_case("-h"; "short help")]
#[test_case("--help"; "long help")]
fn should_use_command_short_help_match(help_flag: &str) {
    let flag = help_flag.trim_start_matches('-');
    let snapshot_name = format!("flag_{flag}");

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(help_flag));
}

#[test_case("cli"; "for cli module")]
#[test_case("collector"; "for collector module")]
#[test_case("expansion"; "for expansion module")]
#[test_case("formatter"; "for formatter module")]
#[test_case("lib"; "for lib module")]
#[test_case("main"; "for main module")]
#[test_case("resolver"; "for resolver module")]
#[test_case("visitor"; "for visitor module")]
fn should_use_command_provide_output(module: &str) {
    let snapshot_name = format!("module_{module}");

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module));
}

#[test_matrix(
  ["cli", "collector", "expansion", "formatter", "lib", "main", "resolver", "visitor"],
  [&["-t"],
   &["-t", "-e"],
   &["-t", "-e", "-d", "1"]
  ]
)]
fn should_use_command_provide_output_for_flags(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("module_{module}_flags_{flags_part}");

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module).args(flags));
}
