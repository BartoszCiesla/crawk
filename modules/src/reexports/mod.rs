mod internal;
mod types;

// Single item re-export
pub use internal::Foo;

// Renamed re-export
pub use internal::Foo as Bar;

// Glob re-export from types
pub use types::*;

// More glob re-exports from internal
pub use internal::{
    Baz, Qux, InternalEnum, InternalTrait,
    INTERNAL_CONST_A, INTERNAL_CONST_B,
    internal_helper_1, internal_helper_2,
};

// Restricted re-export
pub(crate) use internal::Secret;

// Re-export with super import (bring in something from parent)
use super::file_module;

pub fn file_module_greet() -> &'static str {
    file_module::greet()
}

// Crate-absolute import
use crate::nesting;

pub fn nesting_depth() -> u32 {
    nesting::depth()
}

pub fn secret_data() -> &'static str {
    Secret::new().data
}
