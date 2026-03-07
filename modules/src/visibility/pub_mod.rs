use crate::file_module;

pub fn greet() -> &'static str {
    "hello from pub_mod"
}

pub fn file_module_greet() -> &'static str {
    file_module::greet()
}
