use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Item, UseTree};

fn main() {
    let args: Vec<String> = env::args().collect();

    // Handle cargo subcommand invocation (crawk becomes crawk)
    let module_name = if args.len() >= 2 && args[1] == "ls-use" {
        // Invoked as: crawk <module>
        if args.len() < 3 {
            eprintln!("Usage: crawk <module_name>");
            std::process::exit(1);
        }
        &args[2]
    } else if args.len() >= 2 {
        // Invoked directly as: crawk <module>
        &args[1]
    } else {
        eprintln!("Usage: crawk <module_name>");
        std::process::exit(1);
    };

    let current_dir = env::current_dir().expect("Failed to get current directory");
    let src_dir = current_dir.join("src");

    if !src_dir.exists() {
        eprintln!("Error: Not in a Rust project directory (src/ not found)");
        std::process::exit(1);
    }

    // Find the module file
    let module_path = match find_module(&src_dir, module_name) {
        Some(path) => path,
        None => {
            eprintln!("Error: Module '{}' not found", module_name);
            std::process::exit(1);
        }
    };

    println!("Analyzing module: {}", module_name);
    println!("Module path: {}", module_path.display());
    println!();

    // Collect all use statements from the module and its submodules
    let mut use_statements = HashSet::new();
    collect_use_statements(&module_path, &mut use_statements);

    if use_statements.is_empty() {
        println!("No internal crate use statements found.");
    } else {
        println!("Internal crate use statements:");
        let mut sorted_uses: Vec<_> = use_statements.into_iter().collect();
        sorted_uses.sort();
        for use_stmt in sorted_uses {
            println!("  {}", use_stmt);
        }
    }
}

fn find_module(base_dir: &Path, module_name: &str) -> Option<PathBuf> {
    // Check for module_name.rs
    let file_path = base_dir.join(format!("{}.rs", module_name));
    if file_path.exists() {
        return Some(file_path);
    }

    // Check for module_name/mod.rs
    let mod_path = base_dir.join(module_name).join("mod.rs");
    if mod_path.exists() {
        return Some(mod_path);
    }

    // Check in main.rs or lib.rs for inline modules
    for entry_file in &["main.rs", "lib.rs"] {
        let entry_path = base_dir.join(entry_file);
        if entry_path.exists() {
            if let Ok(content) = fs::read_to_string(&entry_path) {
                if let Ok(file) = syn::parse_file(&content) {
                    for item in &file.items {
                        if let Item::Mod(item_mod) = item {
                            if item_mod.ident == module_name {
                                // Found inline module in entry file
                                return Some(entry_path);
                            }
                        }
                    }
                }
            }
        }
    }

    None
}

fn collect_use_statements(path: &Path, use_statements: &mut HashSet<String>) {
    let content = match fs::read_to_string(path) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Warning: Failed to read {}: {}", path.display(), e);
            return;
        }
    };

    let file = match syn::parse_file(&content) {
        Ok(file) => file,
        Err(e) => {
            eprintln!("Warning: Failed to parse {}: {}", path.display(), e);
            return;
        }
    };

    let mut visitor = UseVisitor { use_statements };
    visitor.visit_file(&file);

    // Process submodules
    for item in &file.items {
        if let Item::Mod(item_mod) = item {
            if let Some(submodule_path) = find_submodule(path, &item_mod.ident.to_string()) {
                collect_use_statements(&submodule_path, use_statements);
            }
        }
    }
}

fn find_submodule(parent_path: &Path, submodule_name: &str) -> Option<PathBuf> {
    let parent_dir = parent_path.parent()?;

    // If parent is mod.rs, look in the same directory
    if parent_path.file_name()? == "mod.rs" {
        let base_dir = parent_dir;

        // Check for submodule_name.rs in same directory
        let file_path = base_dir.join(format!("{}.rs", submodule_name));
        if file_path.exists() {
            return Some(file_path);
        }

        // Check for submodule_name/mod.rs
        let mod_path = base_dir.join(submodule_name).join("mod.rs");
        if mod_path.exists() {
            return Some(mod_path);
        }
    } else {
        // Parent is a regular file (e.g., module.rs)
        let module_name = parent_path.file_stem()?.to_str()?;
        let module_dir = parent_dir.join(module_name);

        // Check for module_name/submodule_name.rs
        let file_path = module_dir.join(format!("{}.rs", submodule_name));
        if file_path.exists() {
            return Some(file_path);
        }

        // Check for module_name/submodule_name/mod.rs
        let mod_path = module_dir.join(submodule_name).join("mod.rs");
        if mod_path.exists() {
            return Some(mod_path);
        }
    }

    None
}

struct UseVisitor<'a> {
    use_statements: &'a mut HashSet<String>,
}

impl<'a> Visit<'a> for UseVisitor<'a> {
    fn visit_item_use(&mut self, node: &'a syn::ItemUse) {
        let use_string = format!("use {};", use_tree_to_string(&node.tree));

        // Check if this is an internal crate use (self::, super::, or crate::)
        if is_internal_use(&node.tree) {
            self.use_statements.insert(use_string);
        }

        visit::visit_item_use(self, node);
    }
}

fn is_internal_use(tree: &UseTree) -> bool {
    match tree {
        UseTree::Path(path) => {
            let ident = path.ident.to_string();
            ident == "self" || ident == "super" || ident == "crate"
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            ident == "self" || ident == "super" || ident == "crate"
        }
        UseTree::Rename(rename) => {
            let ident = rename.ident.to_string();
            ident == "self" || ident == "super" || ident == "crate"
        }
        UseTree::Glob(_glob) => {
            // For glob imports, we can't determine from the glob itself,
            // but they're typically preceded by a path
            false
        }
        UseTree::Group(group) => {
            // Check if any item in the group is internal
            group.items.iter().any(|item| is_internal_use(item))
        }
    }
}

fn use_tree_to_string(tree: &UseTree) -> String {
    match tree {
        UseTree::Path(path) => {
            format!("{}::{}", path.ident, use_tree_to_string(&path.tree))
        }
        UseTree::Name(name) => name.ident.to_string(),
        UseTree::Rename(rename) => {
            format!("{} as {}", rename.ident, rename.rename)
        }
        UseTree::Glob(_) => "*".to_string(),
        UseTree::Group(group) => {
            let items: Vec<String> = group
                .items
                .iter()
                .map(|item| use_tree_to_string(item))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

