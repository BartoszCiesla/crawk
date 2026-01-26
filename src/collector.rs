use crate::expansion::is_test_module;
use crate::resolver::{find_submodule, get_src_dir};
use crate::visitor::UseVisitor;
use std::collections::HashSet;
use std::fs;
use std::path::Path;
use syn::visit::Visit;
use syn::Item;

/// Collect use statements from a module file and all its submodules
pub fn collect_use_statements(
    path: &Path,
    use_statements: &mut HashSet<String>,
    include_tests: bool,
    verbose: bool,
    module_path: &[String],
    expand: bool,
    depth: Option<usize>,
) {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            if verbose {
                eprintln!("Warning: Failed to read {}: {}", path.display(), e);
            }
            return;
        }
    };

    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(e) => {
            if verbose {
                eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            }
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
        verbose,
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
                    verbose,
                    &submodule_module_path,
                    expand,
                    depth,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_collect_use_statements_basic() {
        let temp_dir = TempDir::new().unwrap();
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::foo::Bar;
use self::helper::Thing;
use super::parent::Item;
use std::collections::HashMap;
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::foo::{{Bar, Baz}};
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::foo::bar::baz::Thing;
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::foo::Bar;

#[cfg(test)]
mod tests {{
    use crate::test::TestHelper;
}}
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::foo::Bar;

#[cfg(test)]
mod tests {{
    use crate::test::TestHelper;
}}
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            true,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        // Create utils/mod.rs
        let utils_dir = src_dir.join("utils");
        fs::create_dir(&utils_dir).unwrap();
        let utils_mod_rs = utils_dir.join("mod.rs");
        let mut file = fs::File::create(&utils_mod_rs).unwrap();
        writeln!(
            file,
            r#"
pub mod helper;
use crate::foo::Bar;
"#
        )
        .unwrap();

        // Create utils/helper.rs
        let helper_rs = utils_dir.join("helper.rs");
        let mut file = fs::File::create(&helper_rs).unwrap();
        writeln!(
            file,
            r#"
use crate::baz::Qux;
"#
        )
        .unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_mod_rs,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let nonexistent = src_dir.join("nonexistent.rs");

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &nonexistent,
            &mut use_statements,
            false,
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
        let src_dir = temp_dir.path().join("src");
        fs::create_dir(&src_dir).unwrap();

        let utils_rs = src_dir.join("utils.rs");
        let mut file = fs::File::create(&utils_rs).unwrap();
        writeln!(file, "this is not valid rust syntax {{{{").unwrap();

        let mut use_statements = HashSet::new();
        collect_use_statements(
            &utils_rs,
            &mut use_statements,
            false,
            false,
            &["utils".to_string()],
            false,
            None,
        );

        // Should handle gracefully and return empty
        assert!(use_statements.is_empty());
    }
}
