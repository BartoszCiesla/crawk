// Pattern: file-based module (foo.rs)
pub mod file_module;

// Pattern: directory-based module (foo/mod.rs)
pub mod dir_module;

// Pattern: inline sub-modules defined inside a file
pub mod inline_modules;

// Pattern: #[path = "..."] attribute
#[path = "custom_path_target.rs"]
pub mod aliased;

// Pattern: custom_path.rs declares its own #[path] sub-module
pub mod custom_path;

// All visibility modifiers on modules and items
pub mod visibility;

// Deep nesting (3+ levels)
pub mod nesting;

// Re-export patterns
pub mod reexports;

// Mix of inline + file-based in one module
pub mod mixed;

// All visibility combos on items
pub mod item_visibility;

// Comprehensive glob import patterns
pub mod glob_patterns;

// Advanced glob patterns: multi-layer, shadowing, conflicts
pub mod advanced_globs;

// Comprehensive glob showcase with API, models, utils, etc.
pub mod glob_showcase;

// Module with no internal crate dependencies (for testing empty output)
pub mod empty_module;

// Module with only private items (for testing empty glob resolution)
pub mod no_pub_items;

// Module that glob-imports from no_pub_items (glob resolves to nothing)
pub mod uses_empty_glob;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_module_works() {
        assert_eq!(file_module::greet(), "hello from file_module");
    }

    #[test]
    fn dir_module_works() {
        assert_eq!(dir_module::greet(), "hello from dir_module");
        assert_eq!(dir_module::child_file::greet(), "hello from child_file");
        assert_eq!(
            dir_module::child_dir::greet(),
            "hello from child_dir"
        );
    }

    #[test]
    fn inline_modules_work() {
        assert_eq!(
            inline_modules::inner::greet(),
            "hello from inline inner"
        );
        assert_eq!(
            inline_modules::nested::deep::value(),
            42
        );
    }

    #[test]
    fn custom_path_works() {
        assert_eq!(aliased::greet(), "hello from custom_path_target");
    }

    #[test]
    fn nesting_works() {
        assert_eq!(
            nesting::level1::level2::level3::deepest(),
            "hello from level3"
        );
    }

    #[test]
    fn reexports_work() {
        use reexports::*;

        // Single item re-export
        let _ = Foo::new();
        // Renamed re-export
        let _ = Bar::new();
        // Glob re-export - basic types
        let _ = TypeA;
        let _ = TypeB;
        let _ = TypeC;
        let _ = TypeD;
        let _ = TypeE;

        // Glob re-export - additional items
        let baz = Baz::new(42);
        assert_eq!(baz.id, 42);

        let qux = Qux { value: 100 };
        assert_eq!(qux.value, 100);

        // Glob re-exported functions
        assert_eq!(internal_helper_1(), "internal_helper_1");
        assert_eq!(internal_helper_2(), "internal_helper_2");
        assert_eq!(type_helper_a(), "type_helper_a");
        assert_eq!(type_helper_b(), "type_helper_b");

        // Glob re-exported constants
        assert_eq!(INTERNAL_CONST_A, 111);
        assert_eq!(INTERNAL_CONST_B, 222);
        assert_eq!(TYPE_CONST_1, 1001);
        assert_eq!(TYPE_CONST_2, 2002);

        // Glob re-exported enum
        let e = InternalEnum::Variant1;
        assert!(matches!(e, InternalEnum::Variant1));

        // Glob re-exported trait
        let foo = Foo::new();
        let result = foo.internal_method();
        assert!(result.contains("Foo value"));

        // TypeTrait usage
        let type_a = TypeA;
        assert_eq!(type_a.type_method(), "TypeA method");

        // TypeContainer usage
        let container = TypeContainer::new(42);
        assert_eq!(container.inner, 42);

        // Restricted re-export (pub(crate) use)
        let secret = Secret::new();
        assert_eq!(secret.data, "crate-only secret");
    }

    #[test]
    fn mixed_module_works() {
        assert_eq!(mixed::file_child::greet(), "hello from file_child");
        assert_eq!(mixed::inline_child::greet(), "hello from inline_child");
    }

    #[test]
    fn visibility_module_works() {
        // pub mod
        assert_eq!(visibility::pub_mod::greet(), "hello from pub_mod");
        // pub(crate) mod
        assert_eq!(
            visibility::pub_crate_mod::greet(),
            "hello from pub_crate_mod"
        );
    }

    #[test]
    fn item_visibility_works() {
        use item_visibility::{PubCrateEnum, PubCrateTrait};

        let s = item_visibility::PubStruct::default();
        // Read all fields (pub, pub(crate), pub(super))
        assert_eq!(s.pub_field, 0);
        assert_eq!(s.pub_crate_field, 0);
        assert_eq!(s.pub_super_field, 0);

        // All function visibilities
        assert_eq!(item_visibility::pub_function(), "pub_function");
        assert_eq!(item_visibility::pub_crate_function(), "pub_crate_function");
        assert_eq!(item_visibility::pub_super_function(), "pub_super_function");

        // Methods with different visibility
        assert_eq!(s.pub_method(), 0);
        assert_eq!(s.pub_crate_method(), 0);

        // pub(crate) struct
        let pcs = item_visibility::PubCrateStruct { value: 42 };
        assert_eq!(pcs.value, 42);

        // pub(crate) enum
        assert!(matches!(PubCrateEnum::Alpha, PubCrateEnum::Alpha));
        assert!(matches!(PubCrateEnum::Beta, PubCrateEnum::Beta));

        // pub(crate) trait
        assert_eq!(s.crate_only(), 0);

        // Constants and statics
        assert_eq!(item_visibility::PUB_CONST, 100);
        assert_eq!(item_visibility::PUB_CRATE_CONST, "crate-visible");
        assert_eq!(item_visibility::PUB_STATIC, 200);
        assert_eq!(item_visibility::PUB_CRATE_STATIC, "crate-static");

        // Type aliases
        let _: item_visibility::PubAlias = vec![1, 2, 3];
        let _: item_visibility::PubCrateAlias = Some("hello".to_string());

        // Exercise private items via the dedicated function
        let result = item_visibility::exercise_all();
        assert!(result.contains("private_function"));
    }

    #[test]
    fn glob_patterns_work() {
        // Test glob re-exports from utilities
        use glob_patterns::*;

        assert_eq!(helper_a(), "helper_a");
        assert_eq!(helper_b(), "helper_b");
        assert_eq!(helper_c(), "helper_c");
        assert_eq!(UTIL_CONST_1, 100);
        assert_eq!(UTIL_CONST_2, 200);

        // Test glob re-exports from types
        let _alpha = Alpha;
        let _beta = Beta;
        let _gamma = Gamma;
        let _delta = Delta;

        // Test glob re-exports from constants
        assert_eq!(MAX_SIZE, 1024);
        assert_eq!(MIN_SIZE, 16);
        assert_eq!(DEFAULT_TIMEOUT, 30);
        assert_eq!(API_VERSION, "v1.0.0");

        // Test nested glob re-exports
        assert_eq!(outer_func(), "outer_func");
        assert_eq!(deep_func_a(), "deep_a");
        assert_eq!(deep_func_b(), "deep_b");

        // Test using glob-imported traits
        let mut proc = MyProcessor {
            name: "TestProcessor".to_string()
        };
        assert_eq!(proc.greet(), "Hello from TestProcessor");
        proc.process();
        assert_eq!(proc.name, "TestProcessor [processed]");

        // Test demonstration functions
        let demo = glob_patterns::demonstrate_glob_utilities();
        assert!(demo.contains("helper_a"));

        let nested_demo = glob_patterns::demonstrate_nested_glob();
        assert!(nested_demo.contains("outer_func"));
    }

    #[test]
    fn glob_cross_module_usage() {
        use glob_patterns::*;

        // Use items that were glob-imported from other crate modules
        let result = cross_module_glob_usage();
        assert!(result.contains("file_module"));
    }

    #[test]
    fn advanced_globs_multi_layer() {
        use advanced_globs::*;

        // Test multi-layer glob re-exports
        assert_eq!(layer1_function(), "from layer1");
        assert_eq!(layer2_function(), "from layer2");
        assert_eq!(deep_function(), "deep in layer3");
        assert_eq!(DEEP_CONST, 333);

        let deep = DeepStruct { level: 3 };
        assert_eq!(deep.level, 3);

        let demo = demonstrate_multi_layer();
        assert!(demo.contains("layer1"));
        assert!(demo.contains("layer2"));
        assert!(demo.contains("layer3"));
    }

    #[test]
    fn advanced_globs_shadowing() {
        use advanced_globs::*;

        // shadowed_func should be from shadowing module (specific import)
        assert_eq!(shadowed_func(), "shadowing::shadowed_func");

        // unique_original should still be available from glob
        assert_eq!(unique_original(), "unique_original");

        let demo = demonstrate_shadowing();
        assert!(demo.contains("shadowing"));
    }

    #[test]
    fn advanced_globs_traits() {
        use advanced_globs::*;

        let mut multi = MultiImpl {
            buffer: vec!["test".to_string()],
        };

        assert_eq!(multi.provide(), "test");
        multi.consume("data".to_string());
        assert_eq!(multi.buffer.len(), 2);
        assert_eq!(multi.transform("hello"), "HELLO");

        let demo = demonstrate_traits();
        assert!(demo.contains("provided"));
    }

    #[test]
    fn advanced_globs_internal() {
        use advanced_globs::*;

        // internal functions should be available via self glob
        assert_eq!(internal_a(), 1);
        assert_eq!(internal_b(), 2);
        assert_eq!(internal_c(), 3);
        assert_eq!(demonstrate_internal(), 6);
    }

    #[test]
    fn advanced_globs_enum_variants() {
        use advanced_globs::*;

        // Color variants should be directly accessible
        let red = Red;
        let blue = Blue;
        assert_eq!(red, Color::Red);
        assert_eq!(blue, Color::Blue);

        let color = use_color_variants();
        assert_eq!(color, Red);
    }

    #[test]
    fn advanced_globs_renamed() {
        use advanced_globs::*;

        assert_eq!(renamed_func(), "original");
        let _aliased = AliasedStruct;
    }

    #[test]
    fn advanced_globs_prelude() {
        use advanced_globs::prelude::*;

        // Should have access to all prelude items
        assert_eq!(layer1_function(), "from layer1");
        assert_eq!(layer2_function(), "from layer2");
        assert_eq!(deep_function(), "deep in layer3");

        let mut multi = MultiImpl {
            buffer: vec![],
        };
        multi.consume("prelude".to_string());
        assert_eq!(multi.provide(), "prelude");
    }

    #[test]
    fn glob_showcase_api() {
        use glob_showcase::*;

        // Test glob-imported API functions
        assert_eq!(get_data(), "get_data");
        assert!(set_data("test"));
        assert_eq!(count(), 42);
        assert_eq!(API_ENDPOINT, "https://api.example.com");
        assert_eq!(API_TIMEOUT, 5000);
        assert_eq!(API_MAX_RETRIES, 3);

        let demo = demonstrate_api();
        assert!(demo.contains("get_data"));
    }

    #[test]
    fn glob_showcase_models() {
        use glob_showcase::*;

        // Test glob-imported model types
        let user = User {
            id: 1,
            name: "Test".to_string(),
        };
        assert_eq!(user.id, 1);

        let post = Post {
            id: 100,
            content: "Content".to_string(),
        };
        assert_eq!(post.id, 100);

        let comment = Comment {
            id: 50,
            text: "Nice".to_string(),
        };
        assert_eq!(comment.id, 50);

        let _status = Status::Active;
        let _user_id: UserId = 42;

        let demo = demonstrate_models();
        assert!(demo.contains("Alice"));
    }

    #[test]
    fn glob_showcase_database() {
        use glob_showcase::*;

        // Test glob-imported database functions
        assert!(connect("test"));
        assert!(disconnect());
        assert_eq!(execute("SELECT 1"), 0);
        assert!(begin());
        assert!(commit());
        assert_eq!(DEFAULT_PORT, 5432);

        let demo = demonstrate_database();
        assert!(demo.contains("connected"));
    }

    #[test]
    fn glob_showcase_utils() {
        use glob_showcase::*;

        // Test glob-imported utility functions
        assert_eq!(uppercase("hello"), "HELLO");
        assert_eq!(lowercase("WORLD"), "world");
        assert_eq!(add(5, 3), 8);
        assert_eq!(subtract(10, 4), 6);
        assert_eq!(multiply(3, 4), 12);

        let items = vec![1, 2, 3];
        assert_eq!(first(&items), Some(1));
        assert_eq!(last(&items), Some(3));
        assert!(!is_empty(&items));

        let demo = demonstrate_utils();
        assert!(demo.contains("HELLO"));
    }

    #[test]
    fn glob_showcase_errors() {
        use glob_showcase::*;

        let err = NotFoundError {
            message: "Item not found".to_string(),
        };
        assert!(err.message.contains("not found"));

        let val_err = ValidationError {
            field: "email".to_string(),
            message: "Invalid format".to_string(),
        };
        assert_eq!(val_err.field, "email");

        let _kind = ErrorKind::NotFound;
    }

    #[test]
    fn glob_showcase_config() {
        use glob_showcase::*;

        // Test glob-imported config constants
        assert_eq!(HOST, "localhost");
        assert_eq!(PORT, 8080);
        assert_eq!(MAX_CONNECTIONS, 1000);
        assert_eq!(TIMEOUT_MS, 30000);
        assert_eq!(LOG_LEVEL, "info");

        let demo = demonstrate_config();
        assert!(demo.contains("localhost"));
    }

    #[test]
    fn glob_showcase_traits() {
        use glob_showcase::*;

        let user = User {
            id: 99,
            name: "Trait Test".to_string(),
        };

        // Use glob-imported traits
        assert_eq!(user.id(), 99);
        assert_eq!(user.name(), "Trait Test");
        assert!(user.is_valid());

        let demo = demonstrate_traits();
        assert!(demo.contains("valid"));
    }

    #[test]
    fn glob_showcase_prelude() {
        use glob_showcase::prelude::*;

        // Test prelude items (selective re-exports)
        assert_eq!(get_data(), "get_data");

        let user = User {
            id: 1,
            name: "Prelude".to_string(),
        };
        assert_eq!(user.id(), 1);
        assert_eq!(user.name(), "Prelude");

        assert_eq!(add(10, 20), 30);
        assert!(connect("test"));
    }

    #[test]
    fn glob_showcase_comprehensive() {
        let demo = glob_showcase::demonstrate_all();
        assert!(!demo.is_empty());
        assert!(demo.contains("get_data"));
        assert!(demo.contains("Alice"));
    }
}
