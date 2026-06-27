use anyhow::Result;
use vergen_git2::{Build, Cargo, Emitter, Git2, Rustc};

pub fn main() -> Result<()> {
    Emitter::default()
        .add_instructions(&Build::all_build())?
        .add_instructions(&Cargo::all_cargo())?
        .add_instructions(&Git2::all_git())?
        .add_instructions(&Rustc::all_rustc())?
        .emit()?;

    // CRAWK_BUILD_USER (CI/nix) → USER (shell) → "unknown".
    // SysinfoBuilder is omitted to avoid duplicate cargo:rustc-env conflicts;
    // VERGEN_SYSINFO_USER is the only sysinfo key used in version.rs.
    let user = std::env::var("CRAWK_BUILD_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| "unknown".into());
    println!("cargo:rustc-env=CRAWK_BUILD_USER={user}");

    // Override vergen placeholders with env vars (nix flake, CI).
    if let Ok(describe) = std::env::var("CRAWK_GIT_DESCRIBE") {
        println!("cargo:rustc-env=VERGEN_GIT_DESCRIBE={describe}");
    }

    if let Ok(ts) = std::env::var("CRAWK_BUILD_TIMESTAMP") {
        println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP={ts}");
        println!("cargo:rustc-env=VERGEN_BUILD_DATE={}", &ts[..10]);
    }

    Ok(())
}
