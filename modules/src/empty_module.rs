// Module with no internal crate dependencies.
// Used to test the "No internal crate use statements found" code path.

use std::fmt;

pub struct Standalone {
    pub value: u32,
}

impl fmt::Display for Standalone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Standalone({})", self.value)
    }
}

pub fn standalone_fn() -> u32 {
    42
}

pub const STANDALONE_CONST: &str = "no crate deps";
