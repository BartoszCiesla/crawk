use crate::common::{backtrace_filters, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_matrix;

// ============================================================================
// Inline module sub-module tests
// ============================================================================

#[test_matrix(
    ["inline_modules::inner", "inline_modules::nested",
     "inline_modules::nested::deep"],
    [&["-e"],
     &["-r"],
     &["-r", "-e"],
    ]
)]
fn should_modules_use_handle_inline_modules(module: &str, flags: &[&str]) {
    let flags_part = flags
        .iter()
        .map(|f| f.trim_start_matches('-'))
        .collect::<Vec<_>>()
        .join("_");
    let snapshot_name = format!("modules_{}_inline_{flags_part}", module.replace("::", "__"));

    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(
            snapshot_name,
            crawk_modules().arg("use").arg(module).args(flags)
        );
    });
}

// ============================================================================
// File-based `mod` declared inside an inline module
// ============================================================================
//
// `inline_modules::outer` is inline; its file-based child `file_child` must
// resolve to `inline_modules/outer/file_child.rs`, not the decoy at
// `inline_modules/file_child.rs`. Plain assertion (not a snapshot) so this
// fails loudly against the pre-fix resolver instead of silently recording
// the wrong behavior as a snapshot.
#[test]
fn should_resolve_file_based_mod_inside_inline_module_to_correct_directory() {
    let output = crawk_modules()
        .arg("use")
        .arg("inline_modules::outer::file_child")
        .output()
        .expect("failed to run crawk");

    assert!(output.status.success(), "crawk exited with failure");

    let stdout = String::from_utf8(output.stdout).expect("stdout was not valid UTF-8");

    assert!(
        stdout.contains("no_pub_items"),
        "expected the real `outer/file_child.rs` dependency (`no_pub_items`) in output:\n{stdout}"
    );
    assert!(
        !stdout.contains("empty_module"),
        "decoy file `file_child.rs` (wrong directory) was resolved instead:\n{stdout}"
    );
}
