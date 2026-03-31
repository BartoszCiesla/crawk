use crawk::version;

/// Generate the after-help text displayed at the end of the CLI help message
///
/// When `long_help` is `true`, appends build metadata including the build
/// timestamp, target platform, rustc version, build user, and homepage URL.
pub(super) fn generate_after_help(long_help: bool) -> String {
    let name = version::NAME;
    let after_help = format!(
        "Run '{name} --help' for full help message.\n\
         Run '{name} COMMAND --help' for more information on a command.\n\n"
    );

    if long_help {
        let timestamp = version::BUILD_TIMESTAMP;
        let timestamp = timestamp
            .rsplit_once('.')
            .map_or(timestamp, |(before, _)| before);
        let target = version::BUILD_TARGET;
        let rustc = version::RUSTC_VERSION;
        let user = version::BUILD_USER;
        let homepage = version::HOMEPAGE;
        let build_info = format!("Built on {timestamp}Z for {target} ({rustc}) by {user}");

        format!(
            "{after_help}For more about the tool head to {homepage}\n\n\
             {build_info}\n"
        )
    } else {
        after_help
    }
}
