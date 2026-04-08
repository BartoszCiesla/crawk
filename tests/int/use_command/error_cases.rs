use crate::common::crawk;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Parse error — module context in error message
// ============================================================================

#[test]
fn should_fail_with_parse_error_including_module_context() {
    with_settings!({
        filters => vec![
            (env!("CARGO_MANIFEST_DIR"), "[MANIFEST_DIR]"),
        ],
    }, {
        assert_cmd_snapshot!(crawk()
            .arg("-p")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/broken_syntax"))
            .arg("use")
            .arg("broken"));
    });
}

// ============================================================================
// Invalid depth values
// ============================================================================

#[test_case("0"; "zero depth")]
#[test_case("abc"; "non numeric depth")]
fn should_fail_with_invalid_depth(depth: &str) {
    let snapshot_name = format!(
        "invalid_depth_{}",
        depth
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect::<String>()
    );

    assert_cmd_snapshot!(
        snapshot_name,
        crawk().arg("use").arg("lib").arg("-d").arg(depth)
    );
}
