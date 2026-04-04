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

/// File name for library crate root
pub(crate) const LIB_FILE_NAME: &str = "lib.rs";

/// File name for binary crate root
pub(crate) const MAIN_FILE_NAME: &str = "main.rs";
