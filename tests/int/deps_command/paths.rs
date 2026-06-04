use crate::common::{backtrace_filters, crawk_cycles, crawk_paths};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// ============================================================================
// Paths — fixtures/paths (diamond: lib→{a,b}→leaf, sinks: leaf/leaf_via_a/deep)
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

#[test]
fn should_path_plain_format_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
            .arg("-f")
            .arg("plain")
    );
}

#[test]
fn should_path_grouped_format_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
            .arg("-f")
            .arg("grouped")
    );
}

#[test]
fn should_path_dot_format_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("lib")
            .arg("leaf")
            .arg("-f")
            .arg("dot")
    );
}

#[test]
fn should_path_dot_no_path_for_paths_fixture() {
    assert_cmd_snapshot!(
        crawk_paths()
            .arg("deps")
            .arg("--path")
            .arg("leaf")
            .arg("a")
            .arg("-f")
            .arg("dot")
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
