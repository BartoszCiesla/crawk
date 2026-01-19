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
