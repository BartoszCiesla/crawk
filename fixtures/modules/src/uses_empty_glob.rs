// Module that glob-imports from a module with no public items.
// Used to test glob resolution to empty list (lib.rs line 475).

#[allow(unused_imports)]
use crate::no_pub_items::*;

pub fn demonstrate() -> &'static str {
    "uses_empty_glob works"
}
