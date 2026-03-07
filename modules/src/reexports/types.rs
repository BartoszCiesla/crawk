use crate::visibility;

#[derive(Debug, Clone, Copy)]
pub struct TypeA;

#[derive(Debug, Clone, Copy)]
pub struct TypeB;

#[derive(Debug, Clone, Copy)]
pub struct TypeC;

#[derive(Debug, Clone, Copy)]
pub struct TypeD;

#[derive(Debug, Clone, Copy)]
pub struct TypeE;

pub fn visibility_greet() -> &'static str {
    visibility::pub_mod::greet()
}

pub fn type_helper_a() -> &'static str {
    "type_helper_a"
}

pub fn type_helper_b() -> &'static str {
    "type_helper_b"
}

pub const TYPE_CONST_1: u32 = 1001;
pub const TYPE_CONST_2: u32 = 2002;

#[derive(Debug, Clone)]
pub struct TypeContainer<T> {
    pub inner: T,
}

impl<T> TypeContainer<T> {
    pub fn new(value: T) -> Self {
        Self { inner: value }
    }
}

pub trait TypeTrait {
    fn type_method(&self) -> &'static str;
}

impl TypeTrait for TypeA {
    fn type_method(&self) -> &'static str {
        "TypeA method"
    }
}
