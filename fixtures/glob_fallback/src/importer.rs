// Glob import from a module that does not exist in this crate.
// Exercises the resolve_glob fallback path: when resolve_module_path_to_file
// fails, the original glob reference must be preserved unchanged.
use crate::nonexistent_module::*;

pub fn something() {}
