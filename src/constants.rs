/// The library's root module, and the alias that leads a library path handed to
/// discovery: `lib::my_crate` stays the module `my_crate` in crate `my_crate`.
pub(crate) const LIB_ROOT: &str = "lib";

/// Leads the name of a binary target's root module: `bin@<cargo target name>`.
/// Same spelling as the target's `Display` (the `list -T` tag).
pub(crate) const BIN_ROOT_PREFIX: &str = "bin@";

/// Leads the name of an integration test's root module: `test@<cargo target name>`.
/// Same spelling as the target's `Display` (the `list -T` tag).
pub(crate) const TEST_ROOT_PREFIX: &str = "test@";

/// Path qualifier for the crate root
pub(crate) const PATH_QUALIFIER_CRATE: &str = "crate";

/// Path qualifier for the current module
pub(crate) const PATH_QUALIFIER_SELF: &str = "self";

/// Path qualifier for the parent module
pub(crate) const PATH_QUALIFIER_SUPER: &str = "super";

/// Attribute name for conditional compilation
pub(crate) const ATTR_CFG: &str = "cfg";

/// Module name for test modules defined in `#[cfg(test)]` blocks
pub(crate) const MODULE_NAME_TEST: &str = "test";

/// File name for module content defined as a directory
pub(crate) const MODULE_FILE_NAME: &str = "mod.rs";
