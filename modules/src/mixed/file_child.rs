pub fn greet() -> &'static str {
    "hello from file_child"
}

pub fn parent_greet() -> &'static str {
    super::greet()
}

pub fn custom_path_greet() -> &'static str {
    crate::custom_path::greet()
}
