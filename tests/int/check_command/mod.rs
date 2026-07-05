mod basic;
mod config;
mod error_cases;
mod exit_codes;
mod init;
mod layers;
mod overview;

/// Build a throwaway crate with no `crawk.toml`/`.crawk.toml`, so `--init` can
/// succeed. A single `src/lib.rs` (no submodules) keeps the scaffolded
/// `order` list a single entry, which is trivially clean regardless of the
/// (alphabetical) order crawk picks — the success-path tests only care that
/// `--init` writes the file and reports success, not about layer ordering.
///
/// Not itself a `#[test]` fn, so clippy's `allow-expect-in-tests` doesn't
/// cover it; it's test-support scaffolding, so `expect` is fine here.
#[allow(clippy::expect_used)]
pub(super) fn temp_bare_crate() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[package]\nname = \"init_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::create_dir(dir.path().join("src")).expect("create src");
    std::fs::write(dir.path().join("src/lib.rs"), "").expect("write lib.rs");
    dir
}
