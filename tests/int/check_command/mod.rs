mod basic;
mod config;
mod cycles;
mod deny;
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
/// Same idea as [`temp_bare_crate`], but with a real `alpha <-> beta` loop, so
/// `--init` has an existing cycle to freeze. Two top-level modules that
/// reference each other's types — not a containment loop, so `deny-cycles`
/// would report it.
#[allow(clippy::expect_used)]
pub(super) fn temp_cycle_crate() -> tempfile::TempDir {
    let dir = temp_bare_crate();
    let src = dir.path().join("src");
    std::fs::write(
        src.join("lib.rs"),
        "#![allow(dead_code)]\n\nmod alpha;\nmod beta;\n",
    )
    .expect("write lib.rs");
    std::fs::write(
        src.join("alpha.rs"),
        "use crate::beta::BetaType;\n\npub struct AlphaType;\n\nfn _use_beta(_b: BetaType) {}\n",
    )
    .expect("write alpha.rs");
    std::fs::write(
        src.join("beta.rs"),
        "use crate::alpha::AlphaType;\n\npub struct BetaType;\n\nfn _use_alpha(_a: AlphaType) {}\n",
    )
    .expect("write beta.rs");
    dir
}

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
