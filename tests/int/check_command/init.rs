use super::temp_bare_crate;
use crate::common::crawk;
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;

// `--init` on a crate with no existing config: writes `crawk.toml`, exits 0,
// and prints the friendly next-steps message (the one users actually read).
// The temp dir path is filtered out since it's machine/run-specific.
#[test]
fn init_writes_config_and_prints_next_steps() {
    let dir = temp_bare_crate();
    let root = dir.path().to_str().expect("utf8 path");
    let filters = vec![(root, "[ROOT]")];

    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(
            crawk()
                .arg("-p")
                .arg(root)
                .arg("check")
                .arg("--init")
        );
    });

    let written = std::fs::read_to_string(dir.path().join("crawk.toml")).expect("read crawk.toml");
    assert!(written.contains("[[check.layers]]"));
    assert!(written.contains("name = \"init_fixture\""));
}

// `--init` followed by a plain `check` proves the scaffolded config is valid
// and auto-discovered — not just well-formatted text.
#[test]
fn init_then_check_is_clean() {
    let dir = temp_bare_crate();
    let root = dir.path().to_str().expect("utf8 path");

    let init_status = crawk()
        .arg("-p")
        .arg(root)
        .arg("check")
        .arg("--init")
        .status()
        .expect("run crawk check --init");
    assert!(init_status.success());

    let check_status = crawk()
        .arg("-p")
        .arg(root)
        .arg("check")
        .status()
        .expect("run crawk check");
    assert!(check_status.success());
}
