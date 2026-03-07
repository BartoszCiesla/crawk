use crate::common::crawk;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Invalid crate -p path
// ============================================================================
#[test]
fn should_fail_with_nonexistent_path() {
    assert_cmd_snapshot!(
        crawk()
            .arg("-p")
            .arg("/nonexistent/path")
            .arg("use")
            .arg("lib")
    );
}

#[test]
fn should_fail_with_file_as_path() {
    with_settings!({
        filters => vec![
            (env!("CARGO_MANIFEST_DIR"), "[MANIFEST_DIR]"),
        ],
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .arg("use")
            .arg("lib"));
    });
}

// ============================================================================
// Unknown subcommand
// ============================================================================
#[test]
fn should_fail_with_unknown_subcommand() {
    assert_cmd_snapshot!(crawk().arg("unknown"));
}
