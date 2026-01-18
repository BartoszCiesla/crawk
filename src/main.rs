use clap::Parser;
use proc_macro2::Span;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Item, UseTree};

#[derive(Parser, Debug)]
#[command(
    name = "crawk",
    bin_name = "cargo",
    about = "List internal crate use statements from a Rust module"
)]
enum Cargo {
    #[command(name = "ls-use")]
    LsUse(Args),
}

#[derive(Parser, Debug)]
#[command(about = "List internal crate use statements from a Rust module")]
struct Args {
    /// Name of the module to analyze
    module_name: String,

    /// Include test modules in the analysis
    #[arg(short = 't', long = "include-tests")]
    include_tests: bool,

    /// Path to the crate root directory (defaults to current directory)
    #[arg(short = 'p', long = "path")]
    path: Option<PathBuf>,

    /// Show verbose output including crate root, module path, and analysis info
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() {
    // Try to parse as cargo subcommand first
    let args = if env::args().nth(1).as_deref() == Some("ls-use") {
        match Cargo::try_parse() {
            Ok(Cargo::LsUse(args)) => args,
            Err(e) => {
                e.exit();
            }
        }
    } else {
        // Parse as standalone binary
        Args::parse()
    };

    let crate_root = if let Some(path) = &args.path {
        if !path.exists() {
            eprintln!("Error: Provided path '{}' does not exist", path.display());
            std::process::exit(1);
        }
        path.clone()
    } else {
        env::current_dir().expect("Failed to get current directory")
    };

    let src_dir = crate_root.join("src");

    if !src_dir.exists() {
        eprintln!(
            "Error: Not a Rust project directory (src/ not found in {})",
            crate_root.display()
        );
        std::process::exit(1);
    }

    // Find the module file
    let module_path = match find_module(&src_dir, &args.module_name) {
        Some(path) => path,
        None => {
            eprintln!("Error: Module '{}' not found", args.module_name);
            std::process::exit(1);
        }
    };

    if args.verbose {
        println!("Crate root: {}", crate_root.display());
        println!("Analyzing module: {}", args.module_name);
        println!("Module path: {}", module_path.display());
        if !args.include_tests {
            println!("(excluding tests - use --include-tests to include them)");
        }
        println!();
    }

    // Determine the initial module path
    let initial_module_path = get_module_path(&src_dir, &module_path, &args.module_name);

    // Collect all use statements from the module and its submodules
    let mut use_statements = HashSet::new();
    collect_use_statements(
        &module_path,
        &mut use_statements,
        args.include_tests,
        args.verbose,
        &initial_module_path,
    );

    if use_statements.is_empty() {
        if args.verbose {
            println!("No internal crate use statements found.");
        }
    } else {
        let mut sorted_uses: Vec<_> = use_statements.into_iter().collect();
        sorted_uses.sort();
        for use_stmt in sorted_uses {
            println!("{}", use_stmt);
        }
    }
}

fn get_src_dir(path: &Path) -> PathBuf {
    let mut current = path;
    while let Some(parent) = current.parent() {
        if parent.ends_with("src") {
            return parent.to_path_buf();
        }
        current = parent;
    }
    // Fallback: assume current directory has src
    std::env::current_dir()
        .unwrap_or_default()
        .join("src")
}

fn get_module_path(src_dir: &Path, module_file_path: &Path, module_name: &str) -> Vec<String> {
    // If the module is main.rs or lib.rs, it's the crate root
    if let Some(file_name) = module_file_path.file_name() {
        if file_name == "main.rs" || file_name == "lib.rs" {
            return vec![];
        }
    }

    // Build the module path from the file system structure
    let mut path_components = Vec::new();

    // Get the relative path from src_dir to the module file
    if let Ok(relative_path) = module_file_path.strip_prefix(src_dir) {
        for component in relative_path.components() {
            if let Some(component_str) = component.as_os_str().to_str() {
                // Skip "mod.rs" in the path
                if component_str == "mod.rs" {
                    continue;
                }
                // Remove .rs extension
                if let Some(module_part) = component_str.strip_suffix(".rs") {
                    path_components.push(module_part.to_string());
                } else {
                    path_components.push(component_str.to_string());
                }
            }
        }
    } else {
        // Fallback: just use the module name
        path_components.push(module_name.to_string());
    }

    path_components
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

fn collect_use_statements(
    path: &Path,
    use_statements: &mut HashSet<String>,
    include_tests: bool,
    verbose: bool,
    module_path: &[String],
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
                );
            }
        }
    }
}

fn is_test_module(item_mod: &syn::ItemMod) -> bool {
    let module_name = item_mod.ident.to_string();

    // Check if module name is "test" or "tests"
    if module_name == "test" || module_name == "tests" {
        return true;
    }

    // Check for #[cfg(test)] attribute
    for attr in &item_mod.attrs {
        if attr.path().is_ident("cfg") {
            if let Ok(meta_list) = attr.meta.require_list() {
                let tokens = meta_list.tokens.to_string();
                if tokens == "test" {
                    return true;
                }
            }
        }
    }

    false
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
    module_path: Vec<String>,
    src_dir: PathBuf,
    include_tests: bool,
    in_test_module: bool,
}

impl<'a> Visit<'a> for UseVisitor<'a> {
    fn visit_item_use(&mut self, node: &'a syn::ItemUse) {
        // Skip use statements inside test modules unless include_tests is true
        if !self.in_test_module || self.include_tests {
            // Check if this is an internal crate use (self::, super::, or crate::)
            if is_internal_use(&node.tree) {
                // Expand self and super to full module paths
                let mut expanded_tree = expand_use_tree(&node.tree, &self.module_path);

                // Expand globs to explicit items
                expanded_tree = self.expand_globs(&expanded_tree);

                let use_string = format!("use {};", use_tree_to_string(&expanded_tree));
                self.use_statements.insert(use_string);
            }
        }

        visit::visit_item_use(self, node);
    }

    fn visit_item_mod(&mut self, node: &'a syn::ItemMod) {
        let was_in_test = self.in_test_module;

        // Check if this module is a test module
        if !self.include_tests && is_test_module(node) {
            self.in_test_module = true;
        }

        // Continue visiting
        visit::visit_item_mod(self, node);

        // Restore previous state
        self.in_test_module = was_in_test;
    }
}

impl<'a> UseVisitor<'a> {
    fn expand_globs(&self, tree: &UseTree) -> UseTree {
        self.expand_globs_with_path(tree, &[])
    }

    fn expand_globs_with_path(&self, tree: &UseTree, accumulated_path: &[String]) -> UseTree {
        match tree {
            UseTree::Path(path) => {
                let mut new_path = accumulated_path.to_vec();
                new_path.push(path.ident.to_string());

                let expanded_subtree = self.expand_globs_with_path(&path.tree, &new_path);
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expanded_subtree),
                })
            }
            UseTree::Glob(_glob) => {
                // accumulated_path contains the module path (e.g., ["crate", "foo", "bar"])
                // Need to resolve this to a file and get public items
                if let Some(items) = self.resolve_glob_items(accumulated_path) {
                    items
                } else {
                    tree.clone()
                }
            }
            UseTree::Group(group) => {
                let expanded_items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = group
                    .items
                    .iter()
                    .map(|item| self.expand_globs_with_path(item, accumulated_path))
                    .collect();
                UseTree::Group(syn::UseGroup {
                    brace_token: group.brace_token,
                    items: expanded_items,
                })
            }
            _ => tree.clone(),
        }
    }

    fn resolve_glob_items(&self, module_path: &[String]) -> Option<UseTree> {
        // Resolve the module path to a file
        let module_file = self.resolve_module_path_to_file(module_path)?;

        // Parse the file and extract public items
        let public_items = self.extract_public_items(&module_file)?;

        if public_items.is_empty() {
            return None;
        }

        // Create a group with all public items
        let items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = public_items
            .into_iter()
            .map(|name| {
                UseTree::Name(syn::UseName {
                    ident: syn::Ident::new(&name, Span::call_site()),
                })
            })
            .collect();

        Some(UseTree::Group(syn::UseGroup {
            brace_token: syn::token::Brace::default(),
            items,
        }))
    }

    fn resolve_module_path_to_file(&self, module_path: &[String]) -> Option<PathBuf> {
        if module_path.is_empty() {
            return None;
        }

        // First element should be "crate" for internal uses
        if module_path[0] != "crate" {
            return None;
        }

        // Start from src_dir
        let mut current_path = self.src_dir.clone();

        // Navigate through the module path (skip "crate" at index 0)
        for module_name in &module_path[1..] {
            // Try module_name.rs
            let file_path = current_path.join(format!("{}.rs", module_name));
            if file_path.exists() {
                current_path = file_path;
                continue;
            }

            // Try module_name/mod.rs
            let mod_path = current_path.join(module_name).join("mod.rs");
            if mod_path.exists() {
                current_path = mod_path;
                continue;
            }

            // Try to navigate into a directory
            let dir_path = current_path.join(module_name);
            if dir_path.is_dir() {
                current_path = dir_path;
                continue;
            }

            // Module not found
            return None;
        }

        // If current_path is a directory, look for mod.rs
        if current_path.is_dir() {
            let mod_path = current_path.join("mod.rs");
            if mod_path.exists() {
                return Some(mod_path);
            }
        }

        if current_path.is_file() {
            Some(current_path)
        } else {
            None
        }
    }

    fn extract_public_items(&self, file_path: &Path) -> Option<Vec<String>> {
        let content = fs::read_to_string(file_path).ok()?;
        let file = syn::parse_file(&content).ok()?;

        let mut public_items = Vec::new();

        for item in &file.items {
            match item {
                Item::Fn(func) => {
                    if matches!(func.vis, syn::Visibility::Public(_)) {
                        public_items.push(func.sig.ident.to_string());
                    }
                }
                Item::Struct(struct_item) => {
                    if matches!(struct_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(struct_item.ident.to_string());
                    }
                }
                Item::Enum(enum_item) => {
                    if matches!(enum_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(enum_item.ident.to_string());
                    }
                }
                Item::Const(const_item) => {
                    if matches!(const_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(const_item.ident.to_string());
                    }
                }
                Item::Static(static_item) => {
                    if matches!(static_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(static_item.ident.to_string());
                    }
                }
                Item::Type(type_item) => {
                    if matches!(type_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(type_item.ident.to_string());
                    }
                }
                Item::Mod(mod_item) => {
                    if matches!(mod_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(mod_item.ident.to_string());
                    }
                }
                Item::Trait(trait_item) => {
                    if matches!(trait_item.vis, syn::Visibility::Public(_)) {
                        public_items.push(trait_item.ident.to_string());
                    }
                }
                Item::Use(use_item) => {
                    // Handle pub use re-exports
                    if matches!(use_item.vis, syn::Visibility::Public(_)) {
                        self.extract_use_names(&use_item.tree, &mut public_items);
                    }
                }
                _ => {}
            }
        }

        Some(public_items)
    }

    fn extract_use_names(&self, tree: &UseTree, items: &mut Vec<String>) {
        match tree {
            UseTree::Name(name) => {
                items.push(name.ident.to_string());
            }
            UseTree::Rename(rename) => {
                items.push(rename.rename.to_string());
            }
            UseTree::Path(path) => {
                self.extract_use_names(&path.tree, items);
            }
            UseTree::Group(group) => {
                for item in &group.items {
                    self.extract_use_names(item, items);
                }
            }
            UseTree::Glob(_) => {
                // Can't expand nested globs
            }
        }
    }
}

fn expand_use_tree(tree: &UseTree, module_path: &[String]) -> UseTree {
    match tree {
        UseTree::Path(path) => {
            let ident_str = path.ident.to_string();

            if ident_str == "self" {
                // Replace self with crate::module::path
                if module_path.is_empty() {
                    // self at crate root becomes crate
                    UseTree::Path(syn::UsePath {
                        ident: syn::Ident::new("crate", path.ident.span()),
                        colon2_token: path.colon2_token,
                        tree: Box::new(expand_use_tree(&path.tree, module_path)),
                    })
                } else {
                    // Build crate::module::path::rest
                    build_expanded_path(module_path, &path.tree)
                }
            } else if ident_str == "super" {
                // Replace super with parent module path
                if module_path.is_empty() {
                    // super at crate root is invalid, but keep as-is
                    UseTree::Path(syn::UsePath {
                        ident: path.ident.clone(),
                        colon2_token: path.colon2_token,
                        tree: Box::new(expand_use_tree(&path.tree, module_path)),
                    })
                } else {
                    // Go up one level
                    let parent_path = &module_path[..module_path.len() - 1];
                    build_expanded_path(parent_path, &path.tree)
                }
            } else if ident_str == "crate" {
                // crate stays as crate
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expand_use_tree(&path.tree, module_path)),
                })
            } else {
                // Regular path component
                UseTree::Path(syn::UsePath {
                    ident: path.ident.clone(),
                    colon2_token: path.colon2_token,
                    tree: Box::new(expand_use_tree(&path.tree, module_path)),
                })
            }
        }
        UseTree::Name(name) => UseTree::Name(name.clone()),
        UseTree::Rename(rename) => UseTree::Rename(rename.clone()),
        UseTree::Glob(glob) => UseTree::Glob(glob.clone()),
        UseTree::Group(group) => {
            let expanded_items: syn::punctuated::Punctuated<UseTree, syn::Token![,]> = group
                .items
                .iter()
                .map(|item| expand_use_tree(item, module_path))
                .collect();
            UseTree::Group(syn::UseGroup {
                brace_token: group.brace_token,
                items: expanded_items,
            })
        }
    }
}

fn build_expanded_path(module_path: &[String], rest: &UseTree) -> UseTree {
    // Build the path from right to left: rest is the innermost part
    let mut result = expand_use_tree(rest, &[]);

    // Wrap with module path components from right to left
    for module_name in module_path.iter().rev() {
        result = UseTree::Path(syn::UsePath {
            ident: syn::Ident::new(module_name, Span::call_site()),
            colon2_token: syn::Token![::](Span::call_site()),
            tree: Box::new(result),
        });
    }

    // Wrap with crate at the top
    UseTree::Path(syn::UsePath {
        ident: syn::Ident::new("crate", Span::call_site()),
        colon2_token: syn::Token![::](Span::call_site()),
        tree: Box::new(result),
    })
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

