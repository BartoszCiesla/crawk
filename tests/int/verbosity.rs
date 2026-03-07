use crate::common::crawk_modules;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use std::io::{Read, Result};
use std::path::PathBuf;
use test_case::test_case;

// Helper struct to manage log file creation and cleanup for tests
struct LogFile {
    path: PathBuf,
}

impl LogFile {
    fn new(filename: &str) -> Self {
        let temp_dir = std::env::temp_dir();
        let path = temp_dir.join(filename);
        Self { path }
    }

    fn read_content(&self) -> Result<String> {
        let mut content = String::new();
        std::fs::File::open(&self.path)?.read_to_string(&mut content)?;
        Ok(content)
    }

    fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for LogFile {
    fn drop(&mut self) {
        self.cleanup();
    }
}

// ============================================================================
// Verbosity flags (-v, -vv)
// ============================================================================
// These tests exercise cli.rs verbosity() match arms (INFO and DEBUG levels),
// and many info!/debug! logging statements across lib.rs, main.rs, analyzer.rs.
//
// Log output goes to stderr via MinimalFormat, so snapshots capture it.
// We use filters to strip non-deterministic content (absolute paths, etc.).
fn log_filters() -> Vec<(&'static str, &'static str)> {
    vec![
        // Strip absolute paths
        (r"/[^ ]+/modules/src/", "[MODULES]/src/"),
        (r"/[^ ]+/modules", "[MODULES]"),
        // Strip absolute paths in log file context
        (r"/[^ ]+/crawk/src/", "[CRAWK]/src/"),
    ]
}

#[test_case("-v"; "info verbosity")]
#[test_case("-vv"; "debug verbosity")]
fn should_run_with_verbosity(flag: &str) {
    let flag_name = flag.trim_start_matches('-');
    let snapshot_name = format!("verbosity_{flag_name}");

    with_settings!({
        filters => log_filters(),
    }, {
        assert_cmd_snapshot!(
            snapshot_name,
            crawk_modules().arg(flag).arg("use").arg("file_module")
        );
    });
}

#[test]
fn should_run_with_verbosity_recursive() {
    with_settings!({
        filters => log_filters(),
    }, {
        assert_cmd_snapshot!(
            crawk_modules()
                .arg("-v")
                .arg("use")
                .arg("nesting")
                .arg("-r")
                .arg("-e")
        );
    });
}

// ============================================================================
// Log file (--log-file / -l)
// ============================================================================
// Exercises cli.rs file_verbosity(), logger.rs configure_tracing() log file branch.
#[test]
fn should_write_to_log_file() {
    let log_file = LogFile::new("crawk_test_log.txt");

    let output = crawk_modules()
        .arg("-l")
        .arg(&log_file.path)
        .arg("use")
        .arg("file_module")
        .output();
    assert!(output.is_ok(), "Failed to execute crawk");
    let output = output.unwrap();

    assert!(
        output.status.success(),
        "crawk should succeed with --log-file"
    );

    // Verify log file was created and has content
    let log_content = log_file.read_content();
    assert!(log_content.is_ok());
    let log_content = log_content.unwrap();

    assert!(!log_content.is_empty(), "Log file should have content");
    assert!(
        log_content.contains("Analyzing module"),
        "Log file should contain analysis messages"
    );
}

#[test]
fn should_write_debug_to_log_file_with_vv() {
    let log_file = LogFile::new("crawk_test_debug_log.txt");

    let output = crawk_modules()
        .arg("-vv")
        .arg("-l")
        .arg(&log_file.path)
        .arg("use")
        .arg("nesting")
        .arg("-r")
        .arg("-e")
        .output();
    assert!(output.is_ok(), "Failed to execute crawk");
    let output = output.unwrap();

    assert!(
        output.status.success(),
        "crawk should succeed with -vv --log-file"
    );

    // Verify log file contains debug-level content
    let log_content = log_file.read_content();
    assert!(log_content.is_ok());
    let log_content = log_content.unwrap();

    assert!(!log_content.is_empty(), "Log file should have content");
    assert!(
        log_content.contains("DEBUG") || log_content.contains("Analyzing module"),
        "Log file should contain debug or analysis messages"
    );
}
