use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;
use test_case::{test_case, test_matrix};

// ============================================================================
// Output on own modules
// ============================================================================

#[test_case("cli"; "for cli module")]
#[test_case("lib"; "for lib module")]
#[test_case("main"; "for main module")]
#[test_case("parser"; "for parser module")]
#[test_case("discover"; "for discover module")]
#[test_case("reference"; "for reference module")]
#[test_case("nonexisting"; "for nonexisting module")]
fn should_use_command_provide_output(module: &str) {
    let snapshot_name = format!("module_{}", module.replace("::", "__"));

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module));
}

// Test various flag combinations for different modules to ensure consistent output
//
// - crawk and lib shall give the same output for all flags, as they are both crate roots. The only difference
//   will be the module name in the output, which will be "lib" for lib and "crawk" for crawk.
// - main should give the output for binary only.
// - build should give the output for build script only.
// - tests should give the output for test module in the lib only.
#[test_matrix(
        ["crawk", "parser", "discover", "reference", "resolve", "lib", "tests", "main", "build", "cli", "constants", "logger", "version"],
        [&["-t"],
         &["-t", "-e"],
         &["-t", "-e", "-d", "1"],
         &["-r"],
         &["-r", "-t"],
         &["-r", "-t", "-e"],
         &["-r", "-t", "-e", "-G"],
         &["-r", "-t", "-e", "-g", "-G"],
        ]
)]
fn should_use_command_provide_output_for_flags(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("module_{}_flags_{flags_part}", module.replace("::", "__"));

    assert_cmd_snapshot!(snapshot_name, crawk().arg("use").arg(module).args(flags));
}
