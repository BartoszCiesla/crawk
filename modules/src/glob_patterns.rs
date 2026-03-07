// Module showcasing various glob import patterns

// ============================================================================
// Sub-modules with multiple exportable items
// ============================================================================

pub mod utilities {
    pub fn helper_a() -> &'static str { "helper_a" }
    pub fn helper_b() -> &'static str { "helper_b" }
    pub fn helper_c() -> &'static str { "helper_c" }

    pub const UTIL_CONST_1: u32 = 100;
    pub const UTIL_CONST_2: u32 = 200;

    #[derive(Debug, Clone)]
    pub struct UtilityStruct {
        pub value: i32,
    }

    #[derive(Debug)]
    pub enum UtilityEnum {
        OptionA,
        OptionB,
        OptionC,
    }
}

pub mod types {
    #[derive(Debug, Clone, Copy)]
    pub struct Alpha;

    #[derive(Debug, Clone, Copy)]
    pub struct Beta;

    #[derive(Debug, Clone, Copy)]
    pub struct Gamma;

    #[derive(Debug, Clone, Copy)]
    pub struct Delta;

    pub type Result<T> = std::result::Result<T, String>;
    pub type OptionI32 = Option<i32>;
}

pub mod traits {
    pub trait Greetable {
        fn greet(&self) -> String;
    }

    pub trait Processable {
        fn process(&mut self);
    }

    pub trait Serializable {
        fn to_string(&self) -> String;
    }
}

pub mod constants {
    pub const MAX_SIZE: usize = 1024;
    pub const MIN_SIZE: usize = 16;
    pub const DEFAULT_TIMEOUT: u64 = 30;
    pub const API_VERSION: &str = "v1.0.0";
}

// ============================================================================
// Glob re-exports
// ============================================================================

// Re-export all utilities using glob
pub use utilities::*;

// Re-export all types using glob
pub use types::*;

// Re-export all traits using glob
pub use traits::*;

// Re-export all constants using glob
pub use constants::*;

// ============================================================================
// Mixed imports: glob + specific + renamed
// ============================================================================

pub mod mixed_exports {
    // Some items to export
    pub fn func_one() -> u32 { 1 }
    pub fn func_two() -> u32 { 2 }
    pub fn func_three() -> u32 { 3 }
    pub fn special_function() -> u32 { 999 }

    pub struct Item1;
    pub struct Item2;
    pub struct Item3;
}

// Import everything from mixed_exports (local use)
use mixed_exports::{func_one, func_two, func_three, special_function};

// Public re-exports
pub use mixed_exports::func_one as pub_func_one;
pub use mixed_exports::special_function as renamed_special;

pub fn use_mixed_imports() -> u32 {
    func_one() + func_two() + func_three() + special_function()
}

// ============================================================================
// Nested glob patterns
// ============================================================================

pub mod outer {
    pub mod inner {
        pub fn deep_func_a() -> &'static str { "deep_a" }
        pub fn deep_func_b() -> &'static str { "deep_b" }

        pub struct DeepStruct {
            pub data: String,
        }
    }

    // Re-export everything from inner
    pub use inner::*;

    // Also add own items
    pub fn outer_func() -> &'static str { "outer_func" }
}

// Re-export everything from outer (which includes inner's items)
pub use outer::*;

// ============================================================================
// Functions using glob-imported items
// ============================================================================

pub fn demonstrate_glob_utilities() -> String {
    format!(
        "{}, {}, {} | constants: {}, {}, {}",
        helper_a(),
        helper_b(),
        helper_c(),
        UTIL_CONST_1,
        UTIL_CONST_2,
        MAX_SIZE
    )
}

pub fn demonstrate_glob_types() -> String {
    let _a = Alpha;
    let _b = Beta;
    let _g = Gamma;
    let _d = Delta;

    format!("Types: {:?}, {:?}, {:?}, {:?}", _a, _b, _g, _d)
}

pub fn demonstrate_nested_glob() -> String {
    format!(
        "outer: {}, deep: {}, {}",
        outer_func(),
        deep_func_a(),
        deep_func_b()
    )
}

// ============================================================================
// Impl blocks using glob-imported traits
// ============================================================================

pub struct MyProcessor {
    pub name: String,
}

impl Greetable for MyProcessor {
    fn greet(&self) -> String {
        format!("Hello from {}", self.name)
    }
}

impl Processable for MyProcessor {
    fn process(&mut self) {
        self.name.push_str(" [processed]");
    }
}

// ============================================================================
// Glob imports from parent crate
// ============================================================================

// Import from other modules in the crate using globs
use crate::file_module;
use crate::dir_module;

pub fn cross_module_glob_usage() -> String {
    format!(
        "file_module: {}, dir_module: {}, nesting: {}",
        file_module::greet(),
        dir_module::child_file::greet(),
        file_module::nesting_depth()
    )
}

// ============================================================================
// Self-referential glob patterns
// ============================================================================

pub mod self_ref {
    pub fn self_func_1() -> u32 { 1 }
    pub fn self_func_2() -> u32 { 2 }
}

pub use self::self_ref::*;

// ============================================================================
// Complex scenario: multiple globs with potential conflicts
// ============================================================================

pub mod set_a {
    pub fn common_name() -> &'static str { "from_set_a" }
    pub fn unique_to_a() -> &'static str { "unique_a" }
}

pub mod set_b {
    pub fn unique_to_b() -> &'static str { "unique_b" }
    pub struct CommonType;
}

// Import globs from both - no actual conflict here
pub use set_a::*;
pub use set_b::*;

pub fn use_both_sets() -> String {
    format!(
        "{}, {}, {}",
        common_name(),
        unique_to_a(),
        unique_to_b()
    )
}
