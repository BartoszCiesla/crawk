pub fn greet() -> &'static str {
    "hello from child_dir"
}

pub fn root_greeting() -> &'static str {
    crate::file_module::greet()
}
