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
#[test_case("lib"; "for lib module")]
#[test_case("main"; "for main module")]
#[test_case("module"; "for module module")]
#[test_case("module::analyzer"; "for module::analyzer module")]
#[test_case("module::discover"; "for module::discover module")]
#[test_case("module::path"; "for module::path module")]
#[test_case("nonexisting"; "for nonexisting module")]
fn should_use_command_provide_output(module: &str) {
    let snapshot_name = format!("module_{module}");

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module));
}

#[test_matrix(
  ["module", "module::analyzer", "module::discover", "module::path", "module::resolve", "lib", "main", "build", "cli", "constants", "logger", "version"],
  [&["-t"],
   &["-t", "-e"],
   &["-t", "-e", "-d", "1"],
   &["-r"],
   &["-r", "-t"],
   &["-r", "-t", "-e"],
   &["-r", "-t", "-e", "-G"],
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
