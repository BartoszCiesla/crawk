use crate::item_visibility::PUB_STATIC;

#[derive(Debug, Clone, Default)]
pub struct Foo {
    pub value: u32,
}

impl Foo {
    pub fn new() -> Self {
        Self { value: PUB_STATIC }
    }
}

#[derive(Debug, Clone)]
pub struct Secret {
    pub data: &'static str,
}

impl Secret {
    pub fn new() -> Self {
        Self {
            data: "crate-only secret",
        }
    }
}

// Additional items for more comprehensive glob testing
#[derive(Debug, Clone, Copy)]
pub struct Baz {
    pub id: u64,
}

impl Baz {
    pub fn new(id: u64) -> Self {
        Self { id }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Qux {
    pub value: i32,
}

pub fn internal_helper_1() -> &'static str {
    "internal_helper_1"
}

pub fn internal_helper_2() -> &'static str {
    "internal_helper_2"
}

pub const INTERNAL_CONST_A: u32 = 111;
pub const INTERNAL_CONST_B: u32 = 222;

#[derive(Debug)]
pub enum InternalEnum {
    Variant1,
    Variant2(u32),
    Variant3 { x: i32, y: i32 },
}

pub trait InternalTrait {
    fn internal_method(&self) -> String;
}

impl InternalTrait for Foo {
    fn internal_method(&self) -> String {
        format!("Foo value: {}", self.value)
    }
}
