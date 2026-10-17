//! End-to-end checks for a package whose library is not of cargo kind `lib`.
//!
//! `cargo metadata` reports `proc-macro` for `[lib] proc-macro = true` and the
//! crate types themselves (`cdylib`, `rlib`, ...) for an explicit `crate-type`.
//! Each is still the package's one library, so every command must see it.
//!
//! The crate is written into a tempdir instead of living under `fixtures/`:
//! crawk never compiles what it reads, and one throwaway layout serves every
//! manifest variant. A tempdir path would leak into the header of an `insta`
//! snapshot, so the assertions below read stdout directly.

use std::fs;

use tempfile::TempDir;
use test_case::test_case;

use crate::common::crawk;

/// Package `my-macros` with `lib_section` as its `[lib]` table.
///
/// The library holds `foo` and `bar`, both reaching `foo::Bar`, and the
/// integration test target reaches the library through its crate name.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
fn scratch_crate(lib_section: &str) -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path();
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("tests/support")).expect("create tests/support");
    fs::write(
        root.join("Cargo.toml"),
        format!(
            "[package]\nname = \"my-macros\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{lib_section}"
        ),
    )
    .expect("write Cargo.toml");
    fs::write(
        root.join("src/lib.rs"),
        "pub mod foo;\npub mod bar;\nuse crate::foo::Bar;\n#[allow(dead_code)]\nfn touch(_b: Bar) {}\n",
    )
    .expect("write lib.rs");
    fs::write(root.join("src/foo.rs"), "pub struct Bar;\n").expect("write foo.rs");
    fs::write(
        root.join("src/bar.rs"),
        "use crate::foo::Bar;\n#[allow(dead_code)]\npub fn make() -> Bar { Bar }\n",
    )
    .expect("write bar.rs");
    fs::write(root.join("tests/it.rs"), "mod support;\n").expect("write it.rs");
    fs::write(
        root.join("tests/support/mod.rs"),
        "use my_macros::foo::Bar;\n#[allow(dead_code)]\npub fn make() -> Bar { Bar }\n",
    )
    .expect("write support/mod.rs");
    dir
}

/// Runs crawk on `dir` and returns its stdout, asserting it exited with `code`.
///
/// Test-support scaffolding like [`scratch_crate`], hence the `expect`s.
#[allow(clippy::expect_used)]
fn run(dir: &TempDir, code: i32, args: &[&str]) -> String {
    let output = crawk()
        .arg("-p")
        .arg(dir.path())
        .args(args)
        .output()
        .expect("run crawk");
    assert_eq!(
        output.status.code(),
        Some(code),
        "crawk {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("stdout is utf-8")
}

#[test_case("[lib]\nproc-macro = true\n"; "proc-macro")]
#[test_case("[lib]\ncrate-type = [\"cdylib\"]\n"; "cdylib")]
#[test_case("[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n"; "cdylib and rlib")]
#[test_case("[lib]\ncrate-type = [\"staticlib\"]\n"; "staticlib")]
fn should_list_the_library_modules(lib_section: &str) {
    let dir = scratch_crate(lib_section);

    assert_eq!(
        run(&dir, 0, &["list", "-T"]),
        "[lib] bar\n[lib] foo\n[lib] lib\n"
    );
}

#[test_case("[lib]\nproc-macro = true\n"; "proc-macro")]
#[test_case("[lib]\ncrate-type = [\"cdylib\"]\n"; "cdylib")]
#[test_case("[lib]\ncrate-type = [\"cdylib\", \"rlib\"]\n"; "cdylib and rlib")]
#[test_case("[lib]\ncrate-type = [\"staticlib\"]\n"; "staticlib")]
fn should_build_the_dependency_graph(lib_section: &str) {
    let dir = scratch_crate(lib_section);

    assert_eq!(run(&dir, 0, &["deps"]), "bar -> foo\nlib -> foo\n");
}

// `lib` as a module-path root reached the library only through a fallback
// before; it must keep working now that the library target is found directly.
#[test]
fn should_analyze_the_library_root_of_a_proc_macro_crate() {
    let dir = scratch_crate("[lib]\nproc-macro = true\n");

    assert_eq!(run(&dir, 0, &["use", "lib"]), "crate::foo::Bar\n");
    assert_eq!(run(&dir, 0, &["list", "lib"]), "bar\nfoo\nlib\n");
}

// An integration test names the library by its crate name; the reference must
// land on the library module even though the library is a proc-macro crate.
#[test]
fn should_trace_a_test_target_reference_into_a_proc_macro_library() {
    let dir = scratch_crate("[lib]\nproc-macro = true\n");

    assert_eq!(
        run(&dir, 0, &["list", "-T", "-t"]),
        "[lib]     bar\n[lib]     foo\n[lib]     lib\n[test:it] it\n[test:it] support\n"
    );
    assert_eq!(
        run(&dir, 0, &["why", "-t", "support", "foo"]),
        "my_macros::foo::Bar\n"
    );
}

// A `[lib] name` override renames the crate in source: the integration test
// writes `renamed::foo::Bar`, and only a library found at all can own that name.
#[test]
fn should_follow_a_lib_name_override_on_a_proc_macro_crate() {
    let dir = scratch_crate("[lib]\nname = \"renamed\"\nproc-macro = true\n");
    fs::write(
        dir.path().join("tests/support/mod.rs"),
        "use renamed::foo::Bar;\n#[allow(dead_code)]\npub fn make() -> Bar { Bar }\n",
    )
    .unwrap();

    assert_eq!(
        run(&dir, 0, &["why", "-t", "support", "foo"]),
        "renamed::foo::Bar\n"
    );
    assert_eq!(run(&dir, 0, &["deps"]), "bar -> foo\nlib -> foo\n");
}

// Before the fix `deps` came back empty, so `check` passed on any rule: a deny
// rule over a real edge is the case that must now fail.
#[test]
fn should_check_rules_against_a_proc_macro_crate() {
    let dir = scratch_crate("[lib]\nproc-macro = true\n");
    let config = dir.path().join("rules.toml");
    fs::write(&config, "[[check.deny]]\nfrom = \"bar\"\nto = \"foo\"\n").unwrap();

    let stdout = run(&dir, 1, &["check", "-c", config.to_str().unwrap()]);

    assert!(stdout.contains("DENY bar -> foo"), "{stdout}");
}
