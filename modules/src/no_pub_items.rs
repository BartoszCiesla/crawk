#![allow(dead_code)]
// Module with only private items.
// Used to test glob resolution that results in zero public items.

fn private_fn() -> u32 {
    42
}

struct PrivateStruct {
    value: u32,
}

const PRIVATE_CONST: u32 = 100;

enum PrivateEnum {
    A,
    B,
}

// This function exercises the private items so they're not dead code.
pub(crate) fn exercise_private() -> u32 {
    let s = PrivateStruct { value: PRIVATE_CONST };
    let e = match PrivateEnum::A {
        PrivateEnum::A => 1,
        PrivateEnum::B => 2,
    };
    private_fn() + s.value + e
}
