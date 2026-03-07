pub fn greet() -> &'static str {
    "hello from pub_super_mod"
}

pub fn dir_module_greet() -> &'static str {
    crate::dir_module::greet()
}
