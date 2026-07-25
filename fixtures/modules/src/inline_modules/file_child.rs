// Decoy: this is the (wrong) location the pre-fix resolver picks for
// `inline_modules::outer::file_child`. Its own dependency (`empty_module`)
// must NOT show up when analyzing the real submodule.
use crate::empty_module;

pub fn greet() -> &'static str {
    let _ = empty_module::standalone_fn();
    "hello from WRONG decoy file_child"
}
