// File-based sub-module
pub mod file_child;

// Inline sub-module
pub mod inline_child {
    pub fn greet() -> &'static str {
        "hello from inline_child"
    }

    pub fn sibling_greet() -> &'static str {
        super::file_child::greet()
    }
}

pub fn greet() -> &'static str {
    "hello from mixed"
}
