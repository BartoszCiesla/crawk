use crate::common::{backtrace_filters, crawk_cycles, crawk_paths};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Paths — fixtures/paths
// Graph structure (DAG, no cycles):
//   lib → a → {leaf, leaf_via_a, deep_a → deep}
//   lib → b → {leaf, deep_b → deep}
// Shortest paths: lib→leaf (2 paths, len=2), lib→deep (2 paths, len=3)
// Edge cases: direct edge (a→leaf_via_a), no path (leaf→a), self-loop (a→a)
// ============================================================================

#[test]
fn should_path_direct_edge_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("a")
            .arg("leaf_via_a")
    );
}

#[test]
fn should_path_transitive_for_paths_fixture() {
    assert_cmd_snapshot!(crawk_paths().arg("deps").arg("--path").arg("a").arg("deep"));
}

#[test]
fn should_path_length_3_multiple_paths_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("deep")
    );
}

#[test]
fn should_path_multiple_shortest_diamond_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
    );
}

#[test]
fn should_path_no_path_for_paths_fixture() {
    assert_cmd_snapshot!(crawk_paths().arg("deps").arg("--path").arg("leaf").arg("a"));
}

#[test]
fn should_path_source_equals_target() {
    assert_cmd_snapshot!(crawk_paths().arg("deps").arg("--path").arg("a").arg("a"));
}

#[test]
fn should_path_error_unknown_source_for_paths_fixture() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(
            crawk_paths().arg("deps").arg("--path").arg("nonexistent").arg("leaf")
        );
    });
}

#[test]
fn should_path_error_unknown_target_for_paths_fixture() {
    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(
            crawk_paths().arg("deps").arg("--path").arg("lib").arg("nonexistent")
        );
    });
}

#[test]
fn should_path_depth_1_dedup_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
            .arg("-d")
            .arg("1")
    );
}

#[test_case("plain", "lib", "leaf", "should_path_plain_format_for_paths_fixture"; "plain format")]
#[test_case("grouped", "lib", "leaf", "should_path_grouped_format_for_paths_fixture"; "grouped format")]
#[test_case("dot", "lib", "leaf", "should_path_dot_format_for_paths_fixture"; "dot format with paths")]
#[test_case("dot", "leaf", "a", "should_path_dot_no_path_for_paths_fixture"; "dot format no path")]
fn should_path_format_for_paths_fixture(format: &str, source: &str, target: &str, snapshot: &str) {
    assert_cmd_snapshot!(
        snapshot,
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg(source)
            .arg(target)
            .arg("-f")
            .arg(format)
    );
}

// ============================================================================
// Conflict errors — --path cannot combine with --cycles / --orphans / -a
// ============================================================================

#[test]
fn should_error_path_with_cycles() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--path")
            .arg("alpha")
            .arg("beta")
            .arg("--cycles")
    );
}

#[test]
fn should_error_path_with_orphans() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--path")
            .arg("alpha")
            .arg("beta")
            .arg("--orphans")
    );
}

#[test]
fn should_error_path_with_show_apis() {
    assert_cmd_snapshot!(
        crawk_cycles()
            .arg("deps")
            .arg("--path")
            .arg("alpha")
            .arg("beta")
            .arg("-a")
    );
}

// ============================================================================
// --include-tests flag
// ============================================================================

#[test]
fn should_path_with_include_tests_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
            .arg("-t")
    );
}
