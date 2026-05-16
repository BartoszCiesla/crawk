use anyhow::Result;
use vergen_git2::{BuildBuilder, CargoBuilder, Emitter, Git2Builder, RustcBuilder, SysinfoBuilder};

pub fn main() -> Result<()> {
    // Emit all vergen instructions (git2 emits placeholders when .git/ missing)
    Emitter::default()
        .add_instructions(&BuildBuilder::all_build()?)?
        .add_instructions(&CargoBuilder::all_cargo()?)?
        .add_instructions(&Git2Builder::all_git()?)?
        .add_instructions(&RustcBuilder::all_rustc()?)?
        .add_instructions(&SysinfoBuilder::all_sysinfo()?)?
        .emit()?;

    // Override vergen placeholders with env vars from nix flake.
    // Last cargo:rustc-env for a given key wins.
    if let Ok(describe) = std::env::var("CRAWK_GIT_DESCRIBE") {
        println!("cargo:rustc-env=VERGEN_GIT_DESCRIBE={describe}");
    }

    if let Ok(ts) = std::env::var("CRAWK_BUILD_TIMESTAMP") {
        println!("cargo:rustc-env=VERGEN_BUILD_TIMESTAMP={ts}");
        let date = &ts[..10];
        println!("cargo:rustc-env=VERGEN_BUILD_DATE={date}");
    }

    if let Ok(user) = std::env::var("CRAWK_BUILD_USER") {
        println!("cargo:rustc-env=VERGEN_SYSINFO_USER={user}");
    }

    Ok(())
}
