use modules::dir_module;
use modules::file_module;
use modules::inline_modules;
use modules::item_visibility::{PubEnum, PubStruct, PubTrait, PUB_CONST, PUB_STATIC};
use modules::mixed;
use modules::nesting;
// Glob import from reexports to get all re-exported items
use modules::reexports::*;
use modules::visibility;

// ============================================================================
// Extensive glob imports to test dependency analysis
// ============================================================================

// Glob import all constants and types from glob_patterns
use modules::glob_patterns::constants::*;
use modules::glob_patterns::types::*;
use modules::glob_patterns::traits::*;
use modules::glob_patterns::utilities::*;

// Glob import everything from glob_patterns (nested glob re-exports)
use modules::glob_patterns::*;

// Mixed: specific imports alongside glob imports
use modules::glob_patterns::{
    demonstrate_glob_utilities,
    demonstrate_nested_glob,
    renamed_special, // renamed import
};

// Glob import from item_visibility to get multiple items
use modules::item_visibility::*;

// Advanced glob patterns
use modules::advanced_globs::*;

// Comprehensive glob showcase
use modules::glob_showcase::*;

fn main() {
    // File-based module
    println!("{}", file_module::greet());

    // Directory-based module
    println!("{}", dir_module::greet());
    println!("{}", dir_module::child_file::greet());
    println!("{}", dir_module::child_dir::greet());

    // Inline modules
    println!("{}", inline_modules::inner::greet());
    println!("nested deep value: {}", inline_modules::nested::deep::value());

    // Custom path module
    println!("{}", modules::aliased::greet());
    println!("{}", modules::custom_path::greet());

    // Visibility
    println!("{}", visibility::pub_mod::greet());
    let all = visibility::demonstrate_access();
    for g in &all {
        println!("  {g}");
    }

    // Nesting
    println!("{}", nesting::level1::level2::level3::deepest());
    println!("depth at level3: {}", nesting::level1::level2::level3::depth());

    // Re-exports (using glob imports)
    let foo = Foo::new();
    let bar = Bar::new();
    println!("foo: {foo:?}, bar: {bar:?}");
    println!("types: {TypeA:?}, {TypeB:?}, {TypeC:?}, {TypeD:?}, {TypeE:?}");

    // More glob-imported items from reexports
    let baz = Baz::new(999);
    println!("baz: {:?}", baz);
    let qux = Qux { value: -42 };
    println!("qux: {:?}", qux);

    println!("{}, {}", internal_helper_1(), internal_helper_2());
    println!("{}, {}", type_helper_a(), type_helper_b());
    println!("constants: {}, {}, {}, {}",
             INTERNAL_CONST_A, INTERNAL_CONST_B, TYPE_CONST_1, TYPE_CONST_2);

    let enum_val = InternalEnum::Variant2(100);
    println!("enum: {:?}", enum_val);

    let container = TypeContainer::new("hello");
    println!("container: {:?}", container.inner);

    println!("{}", foo.internal_method());
    println!("{}", TypeA.type_method());

    println!("{}", modules::reexports::file_module_greet());
    println!("nesting_depth: {}", modules::reexports::nesting_depth());
    println!("secret: {}", modules::reexports::secret_data());

    // Mixed module
    println!("{}", mixed::greet());
    println!("{}", mixed::file_child::greet());
    println!("{}", mixed::inline_child::greet());

    // Item visibility
    let s = PubStruct::default();
    println!("pub_field: {}", s.pub_field);
    println!("pub_method: {}", s.pub_method());
    println!("trait: {}", s.provided());
    println!("PUB_CONST: {PUB_CONST}");
    println!("PUB_STATIC: {PUB_STATIC}");
    match PubEnum::VariantA {
        PubEnum::VariantA => println!("VariantA"),
        PubEnum::VariantB(n) => println!("VariantB({n})"),
        PubEnum::VariantC { x, y } => println!("VariantC({x}, {y})"),
    }

    // Exercise all visibility levels (pub(crate), pub(super), private)
    println!("exercise: {}", modules::item_visibility::exercise_all());

    println!("\n=== Glob Import Patterns ===");

    // Use glob-imported constants
    println!("MAX_SIZE: {MAX_SIZE}, MIN_SIZE: {MIN_SIZE}");
    println!("API_VERSION: {API_VERSION}, TIMEOUT: {DEFAULT_TIMEOUT}");

    // Use glob-imported utility functions
    println!("{}", helper_a());
    println!("{}", helper_b());
    println!("{}", helper_c());
    println!("UTIL_CONST_1: {UTIL_CONST_1}, UTIL_CONST_2: {UTIL_CONST_2}");

    // Use glob-imported types
    let _alpha = Alpha;
    let _beta = Beta;
    let _gamma = Gamma;
    let _delta = Delta;
    println!("Created types: {:?}, {:?}, {:?}, {:?}", _alpha, _beta, _gamma, _delta);

    // Use glob-imported utility struct
    let util = UtilityStruct { value: 42 };
    println!("UtilityStruct: {:?}", util);

    // Use glob-imported enum
    let enum_val = UtilityEnum::OptionB;
    println!("UtilityEnum: {:?}", enum_val);

    // Use glob-imported traits
    let mut processor = MyProcessor {
        name: "MainProcessor".to_string()
    };
    println!("{}", processor.greet());
    processor.process();
    println!("After processing: {}", processor.name);

    // Use nested glob imports
    println!("outer_func: {}", outer_func());
    println!("deep_func_a: {}, deep_func_b: {}", deep_func_a(), deep_func_b());

    // Use self-referential glob imports
    println!("self_func_1: {}, self_func_2: {}", self_func_1(), self_func_2());

    // Use specific imports that were alongside globs
    println!("demonstrate_glob_utilities: {}", demonstrate_glob_utilities());
    println!("demonstrate_nested_glob: {}", demonstrate_nested_glob());
    println!("renamed_special: {}", renamed_special());

    // Use glob-imported items from item_visibility
    println!("pub_function (via glob): {}", pub_function());

    // Use cross-module glob functionality
    println!("cross_module_glob_usage: {}", cross_module_glob_usage());

    println!("\n=== Advanced Glob Patterns ===");

    // Multi-layer glob re-exports
    println!("layer1_function: {}", layer1_function());
    println!("layer2_function: {}", layer2_function());
    println!("deep_function: {}", deep_function());
    println!("DEEP_CONST: {}", DEEP_CONST);
    println!("{}", demonstrate_multi_layer());

    // Shadowing demonstration
    println!("shadowed_func (specific import wins): {}", shadowed_func());
    println!("unique_original (from glob): {}", unique_original());
    println!("{}", demonstrate_shadowing());

    // Platform-specific glob
    println!("{}", demonstrate_platform());

    // Trait implementations with glob-imported traits
    println!("{}", modules::advanced_globs::demonstrate_traits());

    // Self glob imports
    println!("internal_a: {}, internal_b: {}, internal_c: {}",
             internal_a(), internal_b(), internal_c());
    println!("internal sum: {}", demonstrate_internal());

    // Renamed imports
    println!("renamed_func: {}", renamed_func());
    let _aliased = AliasedStruct;

    // Enum variants via glob
    let color1 = Red;
    let color2 = Blue;
    let color3 = use_color_variants();
    println!("Colors: {:?}, {:?}, {:?}", color1, color2, color3);

    // Prelude usage (all items from prelude)
    println!("prelude layer1: {}", modules::advanced_globs::prelude::layer1_function());

    println!("\n=== Glob Showcase: Comprehensive Patterns ===");

    // API functions (glob-imported)
    println!("API: {}, count: {}", modules::glob_showcase::get_data(), modules::glob_showcase::count());
    println!("API_ENDPOINT: {}", modules::glob_showcase::API_ENDPOINT);
    println!("{}", modules::glob_showcase::demonstrate_api());

    // Models (glob-imported)
    let user = modules::glob_showcase::User {
        id: 1,
        name: "Alice".to_string(),
    };
    let post = modules::glob_showcase::Post {
        id: 100,
        content: "Hello World".to_string(),
    };
    println!("user: {:?}, post: {:?}", user, post);
    println!("{}", modules::glob_showcase::demonstrate_models());

    // Database (glob-imported via nested globs)
    println!("connect: {}", modules::glob_showcase::connect("db://localhost"));
    println!("execute: {}", modules::glob_showcase::execute("SELECT 1"));
    println!("{}", modules::glob_showcase::demonstrate_database());

    // Utils (glob-imported)
    println!("uppercase: {}", modules::glob_showcase::uppercase("test"));
    println!("add: {}", modules::glob_showcase::add(10, 20));
    println!("PI: {}", modules::glob_showcase::PI);
    println!("{}", modules::glob_showcase::demonstrate_utils());

    // Traits (glob-imported and implemented)
    println!("user.id(): {}", user.id());
    println!("user.name(): {}", user.name());
    println!("user.is_valid(): {}", user.is_valid());
    println!("{}", modules::glob_showcase::demonstrate_traits());

    // Config (nested glob re-exports)
    println!("HOST: {}, PORT: {}", modules::glob_showcase::HOST, modules::glob_showcase::PORT);
    println!("LOG_LEVEL: {}", modules::glob_showcase::LOG_LEVEL);
    println!("{}", modules::glob_showcase::demonstrate_config());

    // Errors (glob-imported)
    let err = modules::glob_showcase::NotFoundError {
        message: "test error".to_string(),
    };
    println!("error: {:?}", err);

    // Prelude pattern (from glob_showcase::prelude)
    println!("prelude get_data: {}", modules::glob_showcase::prelude::get_data());

    // Comprehensive demo
    println!("\n{}", modules::glob_showcase::demonstrate_all());

    println!("\n=== All glob patterns exercised ===");
}
