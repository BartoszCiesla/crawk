use crate::nesting::level1::level2::level3;

pub fn greet() -> &'static str {
    "hello from private_mod"
}

pub fn deepest_greet() -> &'static str {
    level3::deepest()
}
