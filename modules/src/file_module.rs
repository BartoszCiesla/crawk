use crate::nesting;

pub fn greet() -> &'static str {
    "hello from file_module"
}

pub fn nesting_depth() -> u32 {
    nesting::depth()
}
