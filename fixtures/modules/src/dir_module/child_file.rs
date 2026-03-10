pub fn greet() -> &'static str {
    "hello from child_file"
}

pub fn parent_greeting() -> &'static str {
    super::greet()
}
