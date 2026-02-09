//! Version and build information for the crawk crate.
//!
//! These constants are populated at build time and provide information
//! about the crate version, build environment, and git state.
//!
//! When the `static-build-config` feature is enabled, static placeholder
//! values are used instead of build-time environment variables.

use const_format::formatcp;

/// The crate name.
pub static NAME: &str = env!("CARGO_PKG_NAME");

/// Short version string (e.g., "0.1.0").
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The crate authors.
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");

/// The crate homepage URL.
pub static HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");

#[cfg(not(feature = "static-build-config"))]
mod build_info {
    /// Build timestamp in ISO 8601 format.
    pub static BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
    /// Build date (YYYY-MM-DD).
    pub static BUILD_DATE: &str = env!("VERGEN_BUILD_DATE");
    /// Target triple the binary was built for.
    pub static BUILD_TARGET: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");
    /// Rust compiler version used for the build.
    pub static RUSTC_VERSION: &str = env!("VERGEN_RUSTC_SEMVER");
    /// Username of the person who built the binary.
    pub static BUILD_USER: &str = env!("VERGEN_SYSINFO_USER");
    /// Git describe output (version tag + commits since + dirty flag).
    pub static GIT_DESCRIBE: &str = env!("VERGEN_GIT_DESCRIBE");
    /// Long version string with git info (e.g., "0.1.0 (v0.1.0-5-gabcdef) 2024-01-15").
    pub const LONG_VERSION: &str = concat!(
        env!("CARGO_PKG_VERSION"),
        " (",
        env!("VERGEN_GIT_DESCRIBE"),
        ") ",
        env!("VERGEN_BUILD_DATE")
    );
}

#[cfg(feature = "static-build-config")]
mod build_info {
    /// Build timestamp in ISO 8601 format.
    pub static BUILD_TIMESTAMP: &str = "2026-02-08T21:14:14.305111052Z";
    /// Build date (YYYY-MM-DD).
    pub static BUILD_DATE: &str = "2026-02-08";
    /// Target triple the binary was built for.
    pub static BUILD_TARGET: &str = "x86_64-unknown-linux-gnu";
    /// Rust compiler version used for the build.
    pub static RUSTC_VERSION: &str = "1.93.0";
    /// Username of the person who built the binary.
    pub static BUILD_USER: &str = "barcie";
    /// Git describe output (version tag + commits since + dirty flag).
    pub static GIT_DESCRIBE: &str = "576e7cc";
    /// Long version string with git info (e.g., "0.1.0 (v0.1.0-5-gabcdef) 2024-01-15").
    pub const LONG_VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), " (576e7cc) 2026-02-08");
}

pub use build_info::{
    BUILD_DATE, BUILD_TARGET, BUILD_TIMESTAMP, BUILD_USER, GIT_DESCRIBE, LONG_VERSION,
    RUSTC_VERSION,
};

/// Full version message with author information.
pub const LONG_VERSION_MESSAGE: &str =
    formatcp!("{}\n\nAuthor: {}", LONG_VERSION, env!("CARGO_PKG_AUTHORS"));
