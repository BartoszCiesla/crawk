pub fn greet() -> &'static str {
    "hello from custom_path_target"
}

pub fn mixed_greet() -> &'static str {
    crate::mixed::greet()
}
