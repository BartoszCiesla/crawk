// Advanced glob patterns: conflicts, shadowing, multi-layer re-exports

// ============================================================================
// Multi-layer glob re-exports
// ============================================================================

pub mod layer1 {
    pub mod layer2 {
        pub mod layer3 {
            pub fn deep_function() -> &'static str {
                "deep in layer3"
            }

            pub const DEEP_CONST: u32 = 333;

            #[derive(Debug)]
            pub struct DeepStruct {
                pub level: u8,
            }
        }

        // Re-export everything from layer3
        pub use layer3::*;

        pub fn layer2_function() -> &'static str {
            "from layer2"
        }
    }

    // Re-export everything from layer2 (which includes layer3)
    pub use layer2::*;

    pub fn layer1_function() -> &'static str {
        "from layer1"
    }
}

// Re-export everything from layer1 (includes layer2 and layer3)
pub use layer1::*;

// ============================================================================
// Shadowing and specificity
// ============================================================================

pub mod original {
    pub fn shadowed_func() -> &'static str {
        "original::shadowed_func"
    }

    pub fn unique_original() -> &'static str {
        "unique_original"
    }

    pub struct OriginalType {
        pub data: String,
    }
}

pub mod shadowing {
    pub fn shadowed_func() -> &'static str {
        "shadowing::shadowed_func"
    }

    pub fn unique_shadowing() -> &'static str {
        "unique_shadowing"
    }
}

// Import from original first
pub use original::*;

// Specific import to override the glob
pub use shadowing::shadowed_func;

// ============================================================================
// Conditional glob imports
// ============================================================================

#[cfg(not(target_os = "windows"))]
pub mod unix_specific {
    pub fn platform_func() -> &'static str {
        "unix_platform"
    }

    pub const PLATFORM_ID: u32 = 1;
}

#[cfg(target_os = "windows")]
pub mod windows_specific {
    pub fn platform_func() -> &'static str {
        "windows_platform"
    }

    pub const PLATFORM_ID: u32 = 2;
}

#[cfg(not(target_os = "windows"))]
pub use unix_specific::*;

#[cfg(target_os = "windows")]
pub use windows_specific::*;

// ============================================================================
// Glob imports with trait implementations
// ============================================================================

pub mod trait_providers {
    pub trait Provider {
        fn provide(&self) -> String;
    }

    pub trait Consumer {
        fn consume(&mut self, data: String);
    }

    pub trait Transformer {
        fn transform(&self, input: &str) -> String;
    }
}

// Glob import all traits
pub use trait_providers::*;

pub struct MultiImpl {
    pub buffer: Vec<String>,
}

impl Provider for MultiImpl {
    fn provide(&self) -> String {
        self.buffer.join(", ")
    }
}

impl Consumer for MultiImpl {
    fn consume(&mut self, data: String) {
        self.buffer.push(data);
    }
}

impl Transformer for MultiImpl {
    fn transform(&self, input: &str) -> String {
        input.to_uppercase()
    }
}

// ============================================================================
// Multiple glob imports from external crate modules
// ============================================================================

// Import globs from various crate modules
use crate::file_module;
use crate::dir_module::child_file;
use crate::visibility;

pub fn use_multiple_crate_globs() -> String {
    format!(
        "file: {}, child: {}, pub_mod: {}",
        file_module::greet(),
        child_file::greet(),
        visibility::pub_mod::greet()
    )
}

// ============================================================================
// Glob with macro imports (macro_use style)
// ============================================================================

pub mod macro_module {
    #[macro_export]
    macro_rules! simple_macro {
        ($x:expr) => {
            format!("macro: {}", $x)
        };
    }

    pub fn helper() -> &'static str {
        "macro_helper"
    }
}

// Glob import the non-macro items
pub use macro_module::*;

// ============================================================================
// Self glob pattern
// ============================================================================

mod internal {
    pub fn internal_a() -> u32 { 1 }
    pub fn internal_b() -> u32 { 2 }
    pub fn internal_c() -> u32 { 3 }
}

pub use self::internal::*;

// ============================================================================
// Renamed glob re-export
// ============================================================================

pub mod aliased_module {
    pub fn original_name() -> &'static str {
        "original"
    }

    pub struct OriginalStruct;
}

// Import and re-rename
pub use aliased_module::original_name as renamed_func;
pub use aliased_module::OriginalStruct as AliasedStruct;

// ============================================================================
// Complex enum with glob pattern
// ============================================================================

pub mod enum_variants {
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub enum Color {
        Red,
        Green,
        Blue,
        Yellow,
        Cyan,
        Magenta,
    }

    // Convenience re-export of variants
    pub use Color::*;
}

// Re-export the enum and its variants
pub use enum_variants::*;

pub fn use_color_variants() -> Color {
    // Can use variants directly due to glob re-export
    Red
}

// ============================================================================
// Prelude-style glob pattern
// ============================================================================

pub mod prelude {
    pub use super::layer1_function;
    pub use super::layer2_function;
    pub use super::deep_function;
    pub use super::Provider;
    pub use super::Consumer;
    pub use super::Transformer;
    pub use super::MultiImpl;
}

// Users can import everything with: use advanced_globs::prelude::*;

// ============================================================================
// Demonstration functions
// ============================================================================

pub fn demonstrate_multi_layer() -> String {
    format!(
        "layer1: {}, layer2: {}, layer3: {} (DEEP_CONST: {})",
        layer1_function(),
        layer2_function(),
        deep_function(),
        DEEP_CONST
    )
}

pub fn demonstrate_shadowing() -> String {
    format!(
        "shadowed (should be from shadowing module): {}, unique_original: {}",
        shadowed_func(),
        unique_original()
    )
}

pub fn demonstrate_platform() -> String {
    format!("platform: {} (ID: {})", platform_func(), PLATFORM_ID)
}

pub fn demonstrate_traits() -> String {
    let mut multi = MultiImpl {
        buffer: vec!["hello".to_string()],
    };

    let provided = multi.provide();
    multi.consume("world".to_string());
    let transformed = multi.transform("test");

    format!(
        "provided: {}, transformed: {}, buffer_len: {}",
        provided,
        transformed,
        multi.buffer.len()
    )
}

pub fn demonstrate_internal() -> u32 {
    internal_a() + internal_b() + internal_c()
}
