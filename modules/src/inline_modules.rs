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
