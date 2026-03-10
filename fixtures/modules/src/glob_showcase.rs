// Comprehensive showcase of all glob patterns

// ============================================================================
// Scenario 1: API module with many functions
// ============================================================================

pub mod api {
    pub fn get_data() -> &'static str { "get_data" }
    pub fn set_data(_value: &str) -> bool { true }
    pub fn delete_data() -> bool { true }
    pub fn update_data(_value: &str) -> bool { true }
    pub fn list_all() -> Vec<&'static str> { vec!["item1", "item2"] }
    pub fn count() -> usize { 42 }
    pub fn exists(_key: &str) -> bool { true }
    pub fn clear_all() -> bool { true }

    pub const API_ENDPOINT: &str = "https://api.example.com";
    pub const API_TIMEOUT: u64 = 5000;
    pub const API_MAX_RETRIES: u32 = 3;  // Renamed to avoid conflict
}

// Glob import all API functions and constants
pub use api::*;

// ============================================================================
// Scenario 2: Models module with many types
// ============================================================================

pub mod models {
    #[derive(Debug, Clone)]
    pub struct User {
        pub id: u64,
        pub name: String,
    }

    #[derive(Debug, Clone)]
    pub struct Post {
        pub id: u64,
        pub content: String,
    }

    #[derive(Debug, Clone)]
    pub struct Comment {
        pub id: u64,
        pub text: String,
    }

    #[derive(Debug, Clone)]
    pub struct Tag {
        pub name: String,
    }

    #[derive(Debug, Clone)]
    pub struct Category {
        pub slug: String,
    }

    #[derive(Debug)]
    pub enum Status {
        Active,
        Inactive,
        Pending,
        Archived,
    }

    pub type UserId = u64;
    pub type PostId = u64;
    pub type CommentId = u64;
}

// Glob import all models
pub use models::*;

// ============================================================================
// Scenario 3: Nested modules with progressive glob re-exports
// ============================================================================

pub mod database {
    pub mod connection {
        pub fn connect(url: &str) -> bool {
            !url.is_empty()
        }

        pub fn disconnect() -> bool { true }

        pub const DEFAULT_PORT: u16 = 5432;
    }

    pub mod query {
        pub fn execute(_sql: &str) -> u32 { 0 }
        pub fn fetch_one(_sql: &str) -> Option<String> { None }
        pub fn fetch_all(_sql: &str) -> Vec<String> { vec![] }

        pub const MAX_QUERY_SIZE: usize = 1024 * 1024;
    }

    pub mod transaction {
        pub fn begin() -> bool { true }
        pub fn commit() -> bool { true }
        pub fn rollback() -> bool { true }
    }

    // Re-export everything from submodules
    pub use connection::*;
    pub use query::*;
    pub use transaction::*;
}

// Re-export everything from database
pub use database::*;

// ============================================================================
// Scenario 4: Utility functions organized by category
// ============================================================================

pub mod utils {
    pub mod string {
        pub fn trim(s: &str) -> &str { s.trim() }
        pub fn uppercase(s: &str) -> String { s.to_uppercase() }
        pub fn lowercase(s: &str) -> String { s.to_lowercase() }
        pub fn reverse(s: &str) -> String { s.chars().rev().collect() }
    }

    pub mod math {
        pub fn add(a: i32, b: i32) -> i32 { a + b }
        pub fn subtract(a: i32, b: i32) -> i32 { a - b }
        pub fn multiply(a: i32, b: i32) -> i32 { a * b }
        pub fn divide(a: i32, b: i32) -> Option<i32> {
            if b != 0 { Some(a / b) } else { None }
        }

        pub const PI: f64 = 3.14159;
        pub const E: f64 = 2.71828;
    }

    pub mod collection {
        pub fn first<T: Clone>(items: &[T]) -> Option<T> {
            items.first().cloned()
        }

        pub fn last<T: Clone>(items: &[T]) -> Option<T> {
            items.last().cloned()
        }

        pub fn is_empty<T>(items: &[T]) -> bool {
            items.is_empty()
        }
    }

    // Selective re-exports with renaming
    pub use string::uppercase as str_upper;
    pub use string::lowercase as str_lower;
    pub use math::{add, subtract, multiply, divide};
    pub use collection::*;

    // Also glob re-export everything
    pub use string::*;
    pub use math::*;
}

// Re-export utils with different patterns
pub use utils::math::*;  // Just math
pub use utils::string::*;  // Just string
pub use utils::collection::*;  // Just collection

// ============================================================================
// Scenario 5: Error types with glob re-exports
// ============================================================================

pub mod errors {
    use std::fmt;

    #[derive(Debug, Clone)]
    pub struct NotFoundError {
        pub message: String,
    }

    #[derive(Debug, Clone)]
    pub struct ValidationError {
        pub field: String,
        pub message: String,
    }

    #[derive(Debug, Clone)]
    pub struct AuthenticationError {
        pub reason: String,
    }

    #[derive(Debug)]
    pub enum ErrorKind {
        NotFound,
        Validation,
        Authentication,
        Internal,
    }

    impl fmt::Display for NotFoundError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Not found: {}", self.message)
        }
    }

    impl fmt::Display for ValidationError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Validation error in {}: {}", self.field, self.message)
        }
    }

    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
}

// Glob re-export all errors
pub use errors::*;

// ============================================================================
// Scenario 6: Configuration with nested globs
// ============================================================================

pub mod config {
    pub mod server {
        pub const HOST: &str = "localhost";
        pub const PORT: u16 = 8080;
        pub const MAX_CONNECTIONS: usize = 1000;
    }

    pub mod client {
        pub const TIMEOUT_MS: u64 = 30000;
        pub const MAX_RETRIES: u32 = 5;
        pub const USER_AGENT: &str = "MyClient/1.0";
    }

    pub mod logging {
        pub const LOG_LEVEL: &str = "info";
        pub const LOG_FILE: &str = "/var/log/app.log";
        pub const MAX_LOG_SIZE_MB: u32 = 100;
    }

    // Nested glob re-exports
    pub mod all {
        pub use super::server::*;
        pub use super::client::*;
        pub use super::logging::*;
    }
}

// Re-export the all module items
pub use config::all::*;

// ============================================================================
// Scenario 7: Traits with default implementations
// ============================================================================

pub mod traits {
    pub trait Identifiable {
        fn id(&self) -> u64;
    }

    pub trait Nameable {
        fn name(&self) -> &str;
    }

    pub trait Timestamped {
        fn created_at(&self) -> u64 { 0 }
        fn updated_at(&self) -> u64 { 0 }
    }

    pub trait Serializable {
        fn to_json(&self) -> String { String::from("{}") }
        fn from_json(_json: &str) -> Self where Self: Sized;
    }

    pub trait Validatable {
        fn is_valid(&self) -> bool { true }
        fn validate(&self) -> Vec<String> { vec![] }
    }
}

// Glob import all traits
pub use traits::*;

// Implement traits for our models
impl Identifiable for User {
    fn id(&self) -> u64 { self.id }
}

impl Nameable for User {
    fn name(&self) -> &str { &self.name }
}

impl Validatable for User {
    fn is_valid(&self) -> bool {
        !self.name.is_empty()
    }
}

// ============================================================================
// Scenario 8: Cross-crate glob imports
// ============================================================================

// Import from various crate modules using globs
use crate::file_module::*;
use crate::inline_modules::*;
use crate::nesting::level1::level2::*;

pub fn use_cross_crate_globs() -> String {
    format!(
        "file: {}, inner: {}, level3: {}",
        greet(),
        inner::greet(),
        level3::deepest()
    )
}

// ============================================================================
// Scenario 9: Prelude pattern (common in libraries)
// ============================================================================

pub mod prelude {
    // Re-export most commonly used items
    pub use super::api::{get_data, set_data, delete_data};
    pub use super::models::{User, Post, Comment};
    pub use super::traits::{Identifiable, Nameable, Validatable};
    pub use super::errors::{NotFoundError, ValidationError};
    pub use super::utils::math::{add, subtract};
    pub use super::database::{connect, disconnect, execute};

    // Glob re-export from super
    pub use super::Status;
    pub use super::ErrorKind;
}

// ============================================================================
// Demonstration and helper functions
// ============================================================================

pub fn demonstrate_api() -> String {
    format!(
        "get: {}, count: {}, endpoint: {}",
        get_data(),
        count(),
        API_ENDPOINT
    )
}

pub fn demonstrate_models() -> String {
    let user = User {
        id: 1,
        name: "Alice".to_string(),
    };

    let post = Post {
        id: 100,
        content: "Hello".to_string(),
    };

    format!("user: {:?}, post: {:?}", user, post)
}

pub fn demonstrate_database() -> String {
    let connected = connect("postgres://localhost");
    let result = execute("SELECT * FROM users");
    format!("connected: {}, result: {}", connected, result)
}

pub fn demonstrate_utils() -> String {
    let upper = uppercase("hello");
    let sum = add(5, 3);
    format!("upper: {}, sum: {}, PI: {}", upper, sum, PI)
}

pub fn demonstrate_traits() -> String {
    let user = User {
        id: 42,
        name: "Bob".to_string(),
    };

    format!(
        "id: {}, name: {}, valid: {}",
        user.id(),
        user.name(),
        user.is_valid()
    )
}

pub fn demonstrate_config() -> String {
    format!(
        "host: {}, port: {}, log: {}",
        HOST,
        PORT,
        LOG_LEVEL
    )
}

pub fn demonstrate_all() -> String {
    format!(
        "{} | {} | {} | {} | {} | {}",
        demonstrate_api(),
        demonstrate_models(),
        demonstrate_database(),
        demonstrate_utils(),
        demonstrate_traits(),
        demonstrate_config()
    )
}
