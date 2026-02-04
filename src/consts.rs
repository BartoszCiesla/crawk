use const_format::formatcp;

pub static CARGO_BIN_NAME: &str = env!("CARGO_PKG_NAME");
pub static BUILD_USER: &str = env!("VERGEN_SYSINFO_USER");
pub static BUILD_TIMESTAMP: &str = env!("VERGEN_BUILD_TIMESTAMP");
pub static BUILD_TARGET: &str = env!("VERGEN_CARGO_TARGET_TRIPLE");
pub static SDK_VERSION: &str = env!("VERGEN_RUSTC_SEMVER");
pub static CARGO_PKG_HOMEPAGE: &str = env!("CARGO_PKG_HOMEPAGE");
pub const VERSION_MESSAGE: &str = concat!(env!("CARGO_PKG_VERSION"),);
pub const LONG_VERSION_STR: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("VERGEN_GIT_DESCRIBE"),
    ") ",
    env!("VERGEN_BUILD_DATE")
);
pub const LONG_VERSION_MESSAGE: &str = formatcp!(
    "{}\n\nAuthor: {}",
    LONG_VERSION_STR,
    env!("CARGO_PKG_AUTHORS")
);
