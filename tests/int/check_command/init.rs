use super::{temp_bare_crate, temp_cycle_crate};
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

// `--init` on a crate that already has a loop freezes it: the scaffold turns
// `deny-cycles` on and grandfathers the existing cycle, so the rule starts green
// and ratchets from there. The message says how many loops were frozen.
#[test]
fn init_freezes_existing_cycles() {
    let dir = temp_cycle_crate();
    let root = dir.path().to_str().expect("utf8 path");
    let filters = vec![(root, "[ROOT]")];

    with_settings!({
        filters => filters,
    }, {
        assert_cmd_snapshot!(crawk().arg("-p").arg(root).arg("check").arg("--init"));
    });

    let written = std::fs::read_to_string(dir.path().join("crawk.toml")).expect("read crawk.toml");
    assert!(written.contains("deny-cycles = true"), "{written}");
    assert!(
        written.contains(r#"modules = ["alpha", "beta"]"#),
        "{written}"
    );
}

// The frozen loop must actually be exempt afterwards. The scaffolded layer order
// is a guess, so `check` still reports LAYER rows — but no CYCLE row.
#[test]
fn init_baseline_silences_the_cycle() {
    let dir = temp_cycle_crate();
    let root = dir.path().to_str().expect("utf8 path");

    let init_status = crawk()
        .arg("-p")
        .arg(root)
        .arg("check")
        .arg("--init")
        .status()
        .expect("run crawk check --init");
    assert!(init_status.success());

    let output = crawk()
        .arg("-p")
        .arg(root)
        .arg("check")
        .output()
        .expect("run crawk check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("CYCLE"), "{stdout}");
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
