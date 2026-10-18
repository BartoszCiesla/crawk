//! Build script: emits the vergen build metadata (and its overrides) that
//! `src/version.rs` reads through `env!`.

use anyhow::Result;
use vergen_git2::{Build, Cargo, Emitter, Git2, Rustc};

fn main() -> Result<()> {
    Emitter::default()
        .add_instructions(&Build::all_build())?
        .add_instructions(&Cargo::all_cargo())?
        .add_instructions(&Rustc::all_rustc())?
        .emit()?;

    emit_git_describe()?;

    // CRAWK_BUILD_USER (CI/nix) → USER (shell) → "unknown".
    // SysinfoBuilder is omitted to avoid duplicate cargo:rustc-env conflicts;
    // VERGEN_SYSINFO_USER is the only sysinfo key used in version.rs.
    let user = std::env::var("CRAWK_BUILD_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=CRAWK_BUILD_USER={user}");

    // Override vergen placeholders with env vars (nix flake, CI).
    if let Ok(ts) = std::env::var("CRAWK_BUILD_TIMESTAMP") {
        println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP={ts}");
        println!(
            "cargo:rustc-env=VERGEN_BUILD_DATE={}",
            ts.get(..10).unwrap_or(ts.as_str())
        );
    }

    Ok(())
}

/// Emits `VERGEN_GIT_DESCRIBE`, which version.rs reads with `env!` and so must
/// always be set: CRAWK_GIT_DESCRIBE (nix flake, CI) → the git repository →
/// the commit cargo packaged the crate from → "unknown".
fn emit_git_describe() -> Result<()> {
    if let Ok(describe) = std::env::var("CRAWK_GIT_DESCRIBE") {
        println!("cargo:rustc-env=VERGEN_GIT_DESCRIBE={describe}");
        return Ok(());
    }

    // A published crate (`cargo install`, docs.rs) is built without a
    // repository. Left to its default, vergen only warns there and sets
    // nothing; `fail_on_error` turns that into an error to fall back on.
    let mut git = Emitter::default();
    if git
        .fail_on_error()
        .add_instructions(&Git2::all_git())
        .is_ok()
    {
        return git.emit();
    }

    let describe = packaged_commit().unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=VERGEN_GIT_DESCRIBE={describe}");
    Ok(())
}

/// Short hash of the commit a published crate was packaged from, which cargo
/// records in `.cargo_vcs_info.json` next to the manifest.
fn packaged_commit() -> Option<String> {
    let info = std::fs::read_to_string(".cargo_vcs_info.json").ok()?;
    let (_, after_key) = info.split_once("\"sha1\"")?;
    let sha = after_key.split('"').nth(1)?;
    sha.get(..7)
        .filter(|short| short.bytes().all(|b| b.is_ascii_hexdigit()))
        .map(str::to_owned)
}
