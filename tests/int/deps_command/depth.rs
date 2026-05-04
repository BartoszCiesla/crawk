use crate::common::{crawk, crawk_modules};
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Depth flag — verifies truncation and self-loop removal
// ============================================================================

#[test_case("1"; "depth 1")]
#[test_case("2"; "depth 2")]
fn should_deps_with_depth_for_own_crate(depth: &str) {
    let snapshot_name = format!("own_depth_{depth}");
    assert_cmd_snapshot!(snapshot_name, crawk().arg("deps").arg("-d").arg(depth));
}

#[test_case("1"; "depth 1")]
#[test_case("2"; "depth 2")]
fn should_deps_with_depth_for_fixture_crate(depth: &str) {
    let snapshot_name = format!("fixture_depth_{depth}");
    assert_cmd_snapshot!(
        snapshot_name,
        crawk_modules().arg("deps").arg("-d").arg(depth)
    );
}

/// Depth 1 must produce a subset of depth 2 edges (at the source level).
///
/// This is a structural invariant: with coarser granularity every edge that
/// appears in depth 1 is either present in depth 2 or collapsed into it.
/// The test is snapshot-based; the invariant is verified by inspecting the
/// snapshot diffs when the implementation changes.
#[test]
fn should_deps_depth_1_produce_top_level_only_for_own_crate() {
    assert_cmd_snapshot!(crawk().arg("deps").arg("-d").arg("1"));
}
