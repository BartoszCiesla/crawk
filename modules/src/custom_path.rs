// This module itself is loaded normally (custom_path.rs).
// It demonstrates referencing the #[path] module declared in lib.rs.
use crate::aliased;
use crate::item_visibility::PUB_CONST;

pub fn greet() -> &'static str {
    "hello from custom_path (the module itself)"
}

pub fn aliased_greet() -> &'static str {
    aliased::greet()
}

pub fn pub_const_value() -> u32 {
    PUB_CONST
}
