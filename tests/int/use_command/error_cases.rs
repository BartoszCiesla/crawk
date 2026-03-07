use crate::common::crawk;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

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
