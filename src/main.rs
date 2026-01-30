use clap::Parser;
use proc_macro2::Span;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use syn::visit::{self, Visit};
use syn::{Item, UseTree};

fn validate_depth(s: &str) -> Result<usize, String> {
    let value: usize = s.parse().map_err(|_| format!("'{}' is not a valid number", s))?;
    if value < 1 {
        Err(String::from("depth must be at least 1"))
    } else {
        Ok(value)
    }
}

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
    /// Module path to analyze (e.g., "utils" or "foo::bar::baz")
    module_path: String,

    /// Include test modules in the analysis
    #[arg(short = 't', long = "include-tests")]
    include_tests: bool,

    /// Path to the crate root directory (defaults to current directory)
    #[arg(short = 'p', long = "path")]
    path: Option<PathBuf>,

    /// Show verbose output including crate root, module path, and analysis info
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Expand grouped imports into individual paths (e.g., a::b::{x, y} -> a::b::x, a::b::y)
    #[arg(short = 'e', long = "expand")]
    expand: bool,

    /// Limit module depth from crate root (e.g., --depth 1 shows crate::x, --depth 2 shows crate::x::y)
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    depth: Option<usize>,
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

    // Parse the module path (e.g., "foo::bar::baz" -> ["foo", "bar", "baz"])
    let module_components: Vec<String> = args
        .module_path
        .split("::")
        .map(|s| s.to_string())
        .collect();

    // Find the module file by navigating through the module hierarchy
    let module_file_path = match find_module_by_path(&src_dir, &module_components) {
        Some(path) => path,
        None => {
            eprintln!("Error: Module '{}' not found", args.module_path);
            std::process::exit(1);
        }
    };

    if args.verbose {
        println!("Crate root: {}", crate_root.display());
        println!("Analyzing module: {}", args.module_path);
        println!("Module file: {}", module_file_path.display());
        if !args.include_tests {
            println!("(excluding tests - use --include-tests to include them)");
        }
        println!();
    }

    // The initial module path is the module components themselves
    let initial_module_path = module_components;

    // Collect all use statements from the module and its submodules
    let mut use_statements = HashSet::new();
    collect_use_statements(
        &module_file_path,
        &mut use_statements,
        args.include_tests,
        args.verbose,
        &initial_module_path,
        args.expand,
        args.depth,
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

fn find_module_by_path(src_dir: &Path, module_path: &[String]) -> Option<PathBuf> {
    if module_path.is_empty() {
        return None;
    }

    // Start from src_dir
    let mut current_dir = src_dir.to_path_buf();

    // Navigate through each component
    for (index, module_name) in module_path.iter().enumerate() {
        let is_last = index == module_path.len() - 1;

        // Try to find the module in the current directory

        // Option 1: module_name.rs in current directory
        let file_path = current_dir.join(format!("{}.rs", module_name));
        if file_path.exists() {
            if is_last {
                return Some(file_path);
            }
            // For non-last components, need to check if there's a directory with the same name
            let module_dir = current_dir.join(module_name);
            if module_dir.is_dir() {
                current_dir = module_dir;
                continue;
            } else {
                // No directory to continue into
                return None;
            }
        }

        // Option 2: module_name/mod.rs
        let mod_file_path = current_dir.join(module_name).join("mod.rs");
        if mod_file_path.exists() {
            if is_last {
                return Some(mod_file_path);
            }
            // Continue into this module's directory
            current_dir = current_dir.join(module_name);
            continue;
        }

        // Module not found
        return None;
    }

    None
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

fn collect_use_statements(
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
    expand: bool,
    depth: Option<usize>,
    verbose: bool,
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

                if self.expand {
                    // Expand groups into individual paths
                    let expanded_paths = expand_use_tree_to_paths(&expanded_tree);
                    for path in expanded_paths {
                        let truncated = truncate_path(&path, self.depth);
                        let without_crate = strip_crate_prefix(&truncated);
                        self.use_statements.insert(format!("{};", without_crate));
                    }
                } else {
                    let use_string = use_tree_to_string(&expanded_tree);
                    let truncated = truncate_path(&use_string, self.depth);
                    let without_crate = strip_crate_prefix(&truncated);
                    self.use_statements.insert(format!("{};", without_crate));
                }
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
        if self.verbose {
            eprintln!("Debug: Attempting to resolve glob for path: {:?}", module_path);
        }

        // Resolve the module path to a file
        let module_file = match self.resolve_module_path_to_file(module_path) {
            Some(f) => {
                if self.verbose {
                    eprintln!("Debug: Resolved glob path to file: {}", f.display());
                }
                f
            }
            None => {
                if self.verbose {
                    eprintln!("Debug: Failed to resolve module path to file");
                }
                return None;
            }
        };

        // Parse the file and extract public items
        let public_items = match self.extract_public_items(&module_file) {
            Some(items) => {
                if self.verbose {
                    eprintln!("Debug: Found {} public items in module", items.len());
                }
                items
            }
            None => {
                if self.verbose {
                    eprintln!("Debug: Failed to extract public items from file");
                }
                return None;
            }
        };

        if public_items.is_empty() {
            if self.verbose {
                eprintln!("Debug: No public items found in module");
            }
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
            if self.verbose {
                eprintln!("Debug: Module path is empty");
            }
            return None;
        }

        // First element should be "crate" for internal uses
        if module_path[0] != "crate" {
            if self.verbose {
                eprintln!("Debug: Module path doesn't start with 'crate': {:?}", module_path);
            }
            return None;
        }

        // Start from src_dir
        let mut current_path = self.src_dir.clone();

        if self.verbose {
            eprintln!("Debug: Starting from src_dir: {}", current_path.display());
        }

        // Navigate through the module path (skip "crate" at index 0)
        for (idx, module_name) in module_path[1..].iter().enumerate() {
            let is_last = idx == module_path.len() - 2; // -2 because we skip "crate" at index 0

            if self.verbose {
                eprintln!("Debug: Looking for module '{}' in {} (is_last={})", module_name, current_path.display(), is_last);
            }

            // Try module_name/mod.rs
            let mod_dir = current_path.join(module_name);
            let mod_path = mod_dir.join("mod.rs");
            if mod_path.exists() {
                if self.verbose {
                    eprintln!("Debug: Found {}", mod_path.display());
                }
                if is_last {
                    // This is the final module, return the mod.rs file
                    current_path = mod_path;
                } else {
                    // Not the final module, continue in the module directory
                    current_path = mod_dir;
                }
                continue;
            }

            // Try module_name.rs
            let file_path = current_path.join(format!("{}.rs", module_name));
            if file_path.exists() {
                if self.verbose {
                    eprintln!("Debug: Found {}", file_path.display());
                }
                if is_last {
                    // This is the final module, return the .rs file
                    current_path = file_path;
                } else {
                    // Not the final module, need to navigate into module_name/ directory
                    let module_dir = current_path.join(module_name);
                    if module_dir.is_dir() {
                        if self.verbose {
                            eprintln!("Debug: Navigating into companion directory {}", module_dir.display());
                        }
                        current_path = module_dir;
                    } else {
                        if self.verbose {
                            eprintln!("Debug: No companion directory found for {}", file_path.display());
                        }
                        return None;
                    }
                }
                continue;
            }

            // Module not found
            if self.verbose {
                eprintln!("Debug: Module '{}' not found at index {}", module_name, idx);
            }
            return None;
        }

        // If current_path is a directory, look for mod.rs
        if current_path.is_dir() {
            let mod_path = current_path.join("mod.rs");
            if mod_path.exists() {
                if self.verbose {
                    eprintln!("Debug: Final path is directory, using mod.rs: {}", mod_path.display());
                }
                return Some(mod_path);
            }
        }

        if current_path.is_file() {
            if self.verbose {
                eprintln!("Debug: Final resolved file: {}", current_path.display());
            }
            Some(current_path)
        } else {
            if self.verbose {
                eprintln!("Debug: Final path is not a file: {}", current_path.display());
            }
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

fn strip_crate_prefix(path: &str) -> String {
    path.strip_prefix("crate::").unwrap_or(path).to_string()
}

fn truncate_path(path: &str, depth: Option<usize>) -> String {
    let depth = match depth {
        Some(d) => d,
        None => return path.to_string(), // No truncation
    };

    // Split by :: to get components
    let parts: Vec<&str> = path.split("::").collect();

    // If the path starts with "crate", count from there
    if parts.first() == Some(&"crate") {
        // depth 1 means crate::x, depth 2 means crate::x::y, etc.
        // So we need depth + 1 components (including "crate")
        let take_count = (depth + 1).min(parts.len());
        parts.iter().take(take_count).map(|s| s.to_string()).collect::<Vec<_>>().join("::")
    } else {
        // For non-crate paths, just take the first 'depth' components
        let take_count = depth.min(parts.len());
        parts.iter().take(take_count).map(|s| s.to_string()).collect::<Vec<_>>().join("::")
    }
}

fn expand_use_tree_to_paths(tree: &UseTree) -> Vec<String> {
    match tree {
        UseTree::Path(path) => {
            let prefix = path.ident.to_string();
            let suffixes = expand_use_tree_to_paths(&path.tree);

            suffixes
                .into_iter()
                .map(|suffix| format!("{}::{}", prefix, suffix))
                .collect()
        }
        UseTree::Name(name) => {
            vec![name.ident.to_string()]
        }
        UseTree::Rename(rename) => {
            vec![format!("{} as {}", rename.ident, rename.rename)]
        }
        UseTree::Glob(_) => {
            vec!["*".to_string()]
        }
        UseTree::Group(group) => {
            let mut all_paths = Vec::new();
            for item in &group.items {
                all_paths.extend(expand_use_tree_to_paths(item));
            }
            all_paths
        }
    }
}
