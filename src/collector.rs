use crate::module::expand::is_test_module;
use crate::module::locate::{find_submodule, get_src_dir};
use crate::visitor::UseVisitor;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use syn::Item;
use syn::visit::Visit;
use tracing::warn;

/// Collect use statements from a module file and all its submodules
#[allow(clippy::implicit_hasher)]
pub fn collect_use_statements(
    path: &Path,
    use_statements: &mut HashSet<String>,
    include_tests: bool,
    module_path: &[String],
    expand: bool,
    depth: Option<usize>,
) {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            warn!("Failed to read {}: {}", path.display(), e);
            return;
        }
    };

    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(e) => {
            warn!("Failed to parse {}: {}", path.display(), e);
            return;
        }
    };

    // Get src directory for module resolution
    let src_dir = get_src_dir(path);

    let mut visitor = UseVisitor {
        use_statements,
        module_path: module_path.to_vec(),
        src_dir,
        include_tests,
        in_test_module: false,
        expand,
        depth,
    };
    visitor.visit_file(&file);

    // Process submodules
    for item in &file.items {
        if let Item::Mod(item_mod) = item {
            // Skip test modules unless include_tests is true
            if !include_tests && is_test_module(item_mod) {
                continue;
            }

            if let Some(submodule_path) = find_submodule(path, &item_mod.ident.to_string()) {
                // Build the new module path for the submodule
                let mut submodule_module_path = module_path.to_vec();
                submodule_module_path.push(item_mod.ident.to_string());

                collect_use_statements(
                    &submodule_path,
                    use_statements,
                    include_tests,
                    &submodule_module_path,
                    expand,
                    depth,
                );
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::constants::DEFAULT_SRC_DIR;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_collect_use_statements_basic() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
use crate::foo::Bar;
use self::helper::Thing;
use super::parent::Item;
use std::collections::HashMap;
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include internal uses but not external (std)
        assert!(use_statements.contains("foo::Bar"));
        assert!(use_statements.contains("utils::helper::Thing"));
        assert!(!use_statements.contains("std::collections::HashMap"));
    }

    #[test]
    fn test_collect_use_statements_with_expand() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
use crate::foo::{{Bar, Baz}};
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            true,
            None,
        );

        // Should expand the group
        assert!(use_statements.contains("foo::Bar"));
        assert!(use_statements.contains("foo::Baz"));
    }

    #[test]
    fn test_collect_use_statements_with_depth() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
use crate::foo::bar::baz::Thing;
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            Some(2),
        );

        // Should truncate to depth 2
        assert!(use_statements.contains("foo::bar"));
    }

    #[test]
    fn test_collect_use_statements_excludes_tests() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
use crate::foo::Bar;

#[cfg(test)]
mod tests {{
    use crate::test::TestHelper;
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include main use but not test use
        assert!(use_statements.contains("foo::Bar"));
        assert!(!use_statements.contains("test::TestHelper"));
    }

    #[test]
    fn test_collect_use_statements_includes_tests() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
use crate::foo::Bar;

#[cfg(test)]
mod tests {{
    use crate::test::TestHelper;
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            true,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include both main use and test use
        assert!(use_statements.contains("foo::Bar"));
        assert!(use_statements.contains("test::TestHelper"));
    }

    #[test]
    fn test_collect_use_statements_with_submodules() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        // Create utils/mod.rs
        let utils_dir = src_dir.join("utils");
        fs::create_dir(&utils_dir).unwrap();
        let utils_mod_rs = utils_dir.join("mod.rs");
        let mut file = fs::File::create(&utils_mod_rs).unwrap();
        writeln!(
            file,
            r"
pub mod helper;
use crate::foo::Bar;
"
        )
        .unwrap();

        // Create utils/helper.rs
        let helper_rs = utils_dir.join("helper.rs");
        let mut file = fs::File::create(&helper_rs).unwrap();
        writeln!(
            file,
            r"
use crate::baz::Qux;
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_mod_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include uses from both parent and submodule
        assert!(use_statements.contains("foo::Bar"));
        assert!(use_statements.contains("baz::Qux"));
    }

    #[test]
    fn test_collect_use_statements_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let nonexistent = src_dir.join("nonexistent.rs");

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &nonexistent,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should handle gracefully and return empty
        assert!(use_statements.is_empty());
    }

    #[test]
    fn test_collect_use_statements_invalid_syntax() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(file, "this is not valid rust syntax {{{{").unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should handle gracefully and return empty
        assert!(use_statements.is_empty());
    }

    #[test]
    fn test_collect_full_path_function_calls() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
fn example() {{
    crate::foo::bar::do_something();
    let x = crate::config::VALUE;
    crate::module::Struct::new();
    self::helper::assist();
    super::parent::check();
    // External paths should not be collected
    std::mem::drop(x);
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include full crate paths from function calls and expressions
        assert!(use_statements.contains("foo::bar::do_something"));
        assert!(use_statements.contains("config::VALUE"));
        assert!(use_statements.contains("module::Struct::new"));
        assert!(use_statements.contains("utils::helper::assist"));
        // super from utils goes to crate root
        assert!(use_statements.contains("parent::check"));
        // External paths should NOT be included
        assert!(!use_statements.contains("std::mem::drop"));
    }

    #[test]
    fn test_collect_full_path_with_depth() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
fn example() {{
    crate::foo::bar::baz::deep_function();
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            Some(2),
        );

        // Should truncate to depth 2
        assert!(use_statements.contains("foo::bar"));
        assert!(!use_statements.contains("foo::bar::baz::deep_function"));
    }

    #[test]
    fn test_collect_full_path_excludes_tests() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
fn production_code() {{
    crate::foo::production_fn();
}}

#[cfg(test)]
mod tests {{
    fn test_something() {{
        crate::test_utils::helper();
    }}
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should include production path but not test path
        assert!(use_statements.contains("foo::production_fn"));
        assert!(!use_statements.contains("test_utils::helper"));
    }

    #[test]
    fn test_collect_type_annotations() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
// Type annotations in various places
fn example(param: crate::types::ParamType) -> crate::types::ReturnType {{
    let x: crate::types::LocalType = todo!();
    let y: Option<crate::types::GenericArg> = None;
    x
}}

struct MyStruct {{
    field: crate::types::FieldType,
}}

type Alias = crate::types::AliasedType;

const CONST: crate::types::ConstType = todo!();

static STATIC: crate::types::StaticType = todo!();
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture all type annotations
        assert!(use_statements.contains("types::ParamType"));
        assert!(use_statements.contains("types::ReturnType"));
        assert!(use_statements.contains("types::LocalType"));
        assert!(use_statements.contains("types::GenericArg"));
        assert!(use_statements.contains("types::FieldType"));
        assert!(use_statements.contains("types::AliasedType"));
        assert!(use_statements.contains("types::ConstType"));
        assert!(use_statements.contains("types::StaticType"));
    }

    #[test]
    fn test_collect_struct_patterns() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
fn example(x: i32) {{
    // Struct pattern in match
    match x {{
        _ if matches!(x, 1) => {{
            let crate::patterns::MyStruct {{ field }} = todo!();
        }}
        _ => {{}}
    }}

    // Tuple struct pattern
    let crate::patterns::TupleStruct(a, b) = todo!();

    // In if-let
    if let crate::patterns::OptionLike {{ value }} = todo!() {{
    }}
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture struct and tuple struct patterns
        assert!(use_statements.contains("patterns::MyStruct"));
        assert!(use_statements.contains("patterns::TupleStruct"));
        assert!(use_statements.contains("patterns::OptionLike"));
    }

    #[test]
    fn test_collect_struct_literals() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
fn example() {{
    // Struct literal construction
    let s = crate::structs::Config {{
        name: "test".to_string(),
        value: 42,
    }};

    // Nested struct literal
    let nested = crate::structs::Outer {{
        inner: crate::structs::Inner {{ data: vec![] }},
    }};

    // Update syntax
    let updated = crate::structs::Config {{
        name: "updated".to_string(),
        ..s
    }};
}}
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture struct literal paths
        assert!(use_statements.contains("structs::Config"));
        assert!(use_statements.contains("structs::Outer"));
        assert!(use_statements.contains("structs::Inner"));
    }

    #[test]
    fn test_collect_trait_bounds() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
// Trait bounds in various places
fn generic_fn<T: crate::traits::MyTrait>(_t: T) {{}}

fn where_clause<T>(_t: T)
where
    T: crate::traits::WhereTrait,
{{}}

struct GenericStruct<T: crate::traits::StructBound> {{
    value: T,
}}

trait LocalTrait: crate::traits::SuperTrait {{}}

fn multi_bound<T: crate::traits::First + crate::traits::Second>(_t: T) {{}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture all trait bounds
        assert!(use_statements.contains("traits::MyTrait"));
        assert!(use_statements.contains("traits::WhereTrait"));
        assert!(use_statements.contains("traits::StructBound"));
        assert!(use_statements.contains("traits::SuperTrait"));
        assert!(use_statements.contains("traits::First"));
        assert!(use_statements.contains("traits::Second"));
    }

    #[test]
    fn test_collect_impl_blocks() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
struct LocalStruct;

// Impl trait for local struct
impl crate::traits::Displayable for LocalStruct {{
    fn display(&self) {{}}
}}

// Impl another trait
impl crate::traits::Serializable for LocalStruct {{
    fn serialize(&self) {{}}
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture traits from impl blocks
        assert!(use_statements.contains("traits::Displayable"));
        assert!(use_statements.contains("traits::Serializable"));
    }

    #[test]
    fn test_collect_macro_invocations() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r"
fn example() {{
    // Macro invocations with crate paths
    crate::macros::my_macro!();
    crate::macros::another_macro!(arg1, arg2);
    self::local_macro!();
}}
"
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture macro paths
        assert!(use_statements.contains("macros::my_macro"));
        assert!(use_statements.contains("macros::another_macro"));
        assert!(use_statements.contains("utils::local_macro"));
    }

    #[test]
    fn test_collect_all_constructs_combined() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join(DEFAULT_SRC_DIR);
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::imports::UsedItem;

struct MyStruct {{
    field: crate::types::FieldType,
}}

impl crate::traits::MyTrait for MyStruct {{
    fn method(&self) -> crate::types::ReturnType {{
        crate::functions::helper();
        let x = crate::structs::Config {{ value: 1 }};
        crate::macros::log!("done");
        todo!()
    }}
}}

fn generic<T: crate::traits::Bound>(param: crate::types::Param) {{
    let crate::patterns::Wrapper(inner) = todo!();
}}
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should capture all types of dependencies
        assert!(use_statements.contains("imports::UsedItem")); // use statement
        assert!(use_statements.contains("types::FieldType")); // struct field type
        assert!(use_statements.contains("traits::MyTrait")); // impl trait
        assert!(use_statements.contains("types::ReturnType")); // return type
        assert!(use_statements.contains("functions::helper")); // function call
        assert!(use_statements.contains("structs::Config")); // struct literal
        assert!(use_statements.contains("macros::log")); // macro
        assert!(use_statements.contains("traits::Bound")); // trait bound
        assert!(use_statements.contains("types::Param")); // param type
        assert!(use_statements.contains("patterns::Wrapper")); // tuple struct pattern
    }
}
