pub mod inner {
    use crate::file_module;

    pub fn greet() -> &'static str {
        "hello from inline inner"
    }

    pub fn file_module_greet() -> &'static str {
        file_module::greet()
    }
}

pub mod nested {
    pub mod deep {
        pub fn value() -> u32 {
            42
        }

        pub fn dir_module_greet() -> &'static str {
            crate::dir_module::greet()
        }
    }
}

mod private_inline {
    pub fn _secret() -> &'static str {
        "you can't see me from outside"
    }
}

// Use item from private inline module within this file
pub fn use_private() -> &'static str {
    private_inline::_secret()
}

pub fn sibling_greet() -> &'static str {
    super::file_module::greet()
}

// File-based `mod` declared inside an inline module. Its real file lives at
// `inline_modules/outer/file_child.rs` (nested under the inline module's own
// name), not `inline_modules/file_child.rs` (regression test for CORR-01).
pub mod outer {
    pub mod file_child;

    pub fn sibling_greet() -> &'static str {
        file_child::greet()
    }
}
