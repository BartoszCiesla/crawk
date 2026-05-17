use std::collections::{HashMap, HashSet};

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::visit::Visit;
use syn::{ItemMod, ItemUse, UseTree};

use tracing::debug;

use crate::constants::{PATH_QUALIFIER_CRATE, PATH_QUALIFIER_SELF, PATH_QUALIFIER_SUPER};
use crate::reference::{GroupItem, PathPrefix, TypeReference};
use crate::utils::has_cfg_test;

/// Resolves relative paths (`self::`, `super::`) to absolute paths (`crate::`).
///
/// Given the current module path, converts relative references to absolute ones:
/// - `self::foo` in module `a::b` becomes `crate::a::b::foo`
/// - `super::foo` in module `a::b` becomes `crate::a::foo`
/// - `super::super::foo` in module `a::b::c` becomes `crate::a::foo`
/// - `crate::foo` stays as `crate::foo`
pub(crate) fn resolve_reference(reference: TypeReference, module_path: &[String]) -> TypeReference {
    match reference.prefix() {
        PathPrefix::SelfModule => {
            let mut new_segments = module_path.to_vec();
            new_segments.extend(reference.segments().iter().cloned());
            reference.with_segments_and_prefix(new_segments, PathPrefix::Crate)
        }
        PathPrefix::Super(levels) => {
            if module_path.len() >= levels {
                let parent_depth = module_path.len() - levels;
                let mut new_segments = module_path[..parent_depth].to_vec();
                new_segments.extend(reference.segments().iter().cloned());
                reference.with_segments_and_prefix(new_segments, PathPrefix::Crate)
            } else {
                // Can't go up that many levels, leave as-is
                debug!(
                    "Cannot resolve super::{} — {levels} levels from module {module_path:?}",
                    reference.segments().join("::")
                );
                reference
            }
        }
        PathPrefix::Crate | PathPrefix::None => reference,
    }
}

/// Type references collected from a single module, grouped by syntactic role.
///
/// Each category represents a distinct kind of dependency signal:
/// - `use_statements` — explicit imports (`use crate::foo::Bar`)
/// - `type_refs` — type-level dependencies (annotations, trait bounds, impl traits)
/// - `value_refs` — value-level dependencies (calls, struct construction, patterns)
/// - `macro_calls` — macro invocations (semantically opaque to static analysis)
pub(super) struct CollectedReferences {
    pub use_statements: Vec<TypeReference>,
    pub type_refs: Vec<TypeReference>,
    pub value_refs: Vec<TypeReference>,
    pub macro_calls: Vec<TypeReference>,
}

impl CollectedReferences {
    /// Creates a new empty [`CollectedReferences`] with all categories initialized.
    const fn new() -> Self {
        Self {
            use_statements: Vec::new(),
            type_refs: Vec::new(),
            value_refs: Vec::new(),
            macro_calls: Vec::new(),
        }
    }

    /// Iterates over all references in a consistent order.
    pub(super) fn all(&self) -> impl Iterator<Item = &TypeReference> {
        self.use_statements
            .iter()
            .chain(self.type_refs.iter())
            .chain(self.value_refs.iter())
            .chain(self.macro_calls.iter())
    }
}

/// Visitor for extracting type references from module AST.
pub(super) struct ModuleVisitor {
    /// Module name for filtering and identification (e.g., "foo::bar").
    /// Used to match against nested module declarations. Empty string means analyze all modules.
    module_name: String,

    /// Module path as segments for resolving relative paths (e.g., `["foo", "bar"]`).
    /// Used to convert `self::` and `super::` references to absolute `crate::` paths.
    module_path: Vec<String>,

    /// Known direct child module names for this module.
    /// Used to recognise bare internal paths (e.g. `format::deps_cmd::build_edges()`)
    /// in expressions, types, patterns, and macro invocations.
    children: HashSet<String>,

    /// Collected type references found in this module, grouped by syntactic role.
    /// All relative paths are resolved to absolute paths before being added.
    pub(super) references: CollectedReferences,

    /// Track if we're currently visiting inside a test module.
    /// Updated as we traverse nested modules to filter test-only references.
    in_test_module: bool,

    /// Package name for recognising same-crate imports with bare package prefix.
    /// E.g., `use crawk::version` where `crawk` is the package name.
    package_name: Option<String>,

    /// Maps imported short names to `(prefix, path_segments)`.
    /// Built on-the-fly from `use` statements during traversal.
    /// - `use crate::version;` → `"version" → (Crate, ["version"])`
    /// - `use crawk::version;` → `"version" → (None, ["crawk", "version"])`
    imported_modules: HashMap<String, (PathPrefix, Vec<String>)>,
}

impl ModuleVisitor {
    /// Creates a new [`ModuleVisitor`] scoped to `module_name`.
    ///
    /// Pass an empty string to visit all modules without filtering.
    /// The module path for relative-reference resolution is derived from `module_name`
    /// by splitting on `::`.
    ///
    /// `children` contains the names of direct child modules declared via `mod` in
    /// this module. Bare paths whose first segment matches a child are recognised
    /// as internal references (e.g. `format::deps_cmd::build_edges()`).
    ///
    /// `package_name` enables resolution of imports via the package name prefix
    /// (e.g., `use crawk::version` when `crawk` is the package). Pass `None`
    /// to disable import-aware token resolution.
    pub(super) fn new(
        module_name: impl Into<String>,
        children: HashSet<String>,
        package_name: Option<String>,
    ) -> Self {
        let module_name = module_name.into();
        let module_path: Vec<String> = if module_name.is_empty() {
            vec![]
        } else {
            module_name.split("::").map(String::from).collect()
        };

        Self {
            module_name,
            module_path,
            children,
            references: CollectedReferences::new(),
            in_test_module: false,
            package_name,
            imported_modules: HashMap::new(),
        }
    }

    /// Checks if a syn::Path is an internal crate reference.
    /// Returns true if the path starts with `crate::`, `self::`, `super::`,
    /// or a known child module name.
    fn is_internal_path(&self, path: &syn::Path) -> bool {
        path.segments.first().is_some_and(|first_segment| {
            let ident = first_segment.ident.to_string();
            matches!(
                ident.as_str(),
                PATH_QUALIFIER_CRATE | PATH_QUALIFIER_SELF | PATH_QUALIFIER_SUPER
            ) || self.children.contains(&ident)
        })
    }

    /// Builds a TypeReference from a syn::Path if it's an internal crate reference.
    fn build_reference(&self, path: &syn::Path) -> Option<TypeReference> {
        if !self.is_internal_path(path) {
            return None;
        }

        let mut segments = Vec::new();
        let mut path_prefix = PathPrefix::None;

        for (i, segment) in path.segments.iter().enumerate() {
            let ident = segment.ident.to_string();

            // Handle special prefixes at the start
            if i == 0 {
                match ident.as_str() {
                    PATH_QUALIFIER_CRATE => {
                        path_prefix = PathPrefix::Crate;
                        continue;
                    }
                    PATH_QUALIFIER_SELF => {
                        path_prefix = PathPrefix::SelfModule;
                        continue;
                    }
                    PATH_QUALIFIER_SUPER => {
                        path_prefix = PathPrefix::Super(1);
                        continue;
                    }
                    _ => {}
                }
            } else if ident == PATH_QUALIFIER_SUPER {
                // Handle chained super::
                let levels = match path_prefix {
                    PathPrefix::Super(n) => n + 1,
                    _ => 1,
                };
                path_prefix = PathPrefix::Super(levels);
                continue;
            }

            segments.push(ident);
        }

        if segments.is_empty() {
            return None;
        }

        Some(resolve_reference(
            TypeReference::new(segments).with_prefix(path_prefix),
            &self.module_path,
        ))
    }

    /// Builds a [`TypeReference`] from raw path segments (e.g. `["version", "VERSION"]`).
    ///
    /// Used for paths extracted from opaque token streams (macro arguments, attribute
    /// values) where no `syn::Path` is available. Applies the same internal-path
    /// filtering and prefix resolution as [`build_reference`].
    fn build_reference_from_segments(&self, segments: &[String]) -> Option<TypeReference> {
        if segments.is_empty() {
            return None;
        }

        let first = &segments[0];
        let is_internal = matches!(
            first.as_str(),
            PATH_QUALIFIER_CRATE | PATH_QUALIFIER_SELF | PATH_QUALIFIER_SUPER
        ) || self.children.contains(first);

        if !is_internal {
            return None;
        }

        let mut prefix = PathPrefix::None;
        let mut start = 0;

        match segments[0].as_str() {
            PATH_QUALIFIER_CRATE => {
                prefix = PathPrefix::Crate;
                start = 1;
            }
            PATH_QUALIFIER_SELF => {
                prefix = PathPrefix::SelfModule;
                start = 1;
            }
            PATH_QUALIFIER_SUPER => {
                let levels = segments
                    .iter()
                    .take_while(|s| s.as_str() == PATH_QUALIFIER_SUPER)
                    .count();
                prefix = PathPrefix::Super(levels);
                start = levels;
            }
            _ => {}
        }

        let real_segments: Vec<String> = segments[start..].to_vec();
        if real_segments.is_empty() {
            return None;
        }

        Some(resolve_reference(
            TypeReference::new(real_segments).with_prefix(prefix),
            &self.module_path,
        ))
    }

    /// Records an import in `imported_modules` if it originates from the current crate.
    ///
    /// Recognises imports via `crate::`, `self::`, `super::`, and bare package name
    /// (e.g., `use crawk::version` where `crawk` is `self.package_name`).
    /// Stores `(prefix, segments)` so that `resolve_via_import` can reconstruct
    /// the reference with the correct prefix:
    /// - `use crate::version` → `(Crate, ["version"])` → `crate::version::NAME`
    /// - `use crawk::version` → `(None, ["crawk", "version"])` → `crawk::version::NAME`
    fn try_record_import(
        &mut self,
        imported_name: String,
        segments: &[String],
        prefix: PathPrefix,
    ) {
        let entry = match prefix {
            PathPrefix::Crate => Some((PathPrefix::Crate, segments.to_vec())),
            PathPrefix::SelfModule => {
                let mut abs = self.module_path.clone();
                abs.extend(segments.iter().cloned());
                Some((PathPrefix::Crate, abs))
            }
            PathPrefix::Super(levels) => {
                if self.module_path.len() >= levels {
                    let mut abs = self.module_path[..self.module_path.len() - levels].to_vec();
                    abs.extend(segments.iter().cloned());
                    Some((PathPrefix::Crate, abs))
                } else {
                    None
                }
            }
            PathPrefix::None => self.package_name.as_ref().and_then(|pkg| {
                if segments.first().is_some_and(|s| s == pkg) {
                    // Keep full path including package name for cross-target display
                    Some((PathPrefix::None, segments.to_vec()))
                } else {
                    None
                }
            }),
        };

        if let Some((pfx, path)) = entry {
            if !path.is_empty() {
                self.imported_modules.insert(imported_name, (pfx, path));
            }
        }
    }

    /// Resolves a path found in tokens via the import alias map.
    ///
    /// If the first segment matches an imported name, replaces it with the stored
    /// path and prefix. Cross-target imports (via package name) keep `PathPrefix::None`
    /// so they display as `crawk::version::NAME`; same-target imports use `PathPrefix::Crate`
    /// so they display as `crate::version::NAME`.
    fn resolve_via_import(&self, segments: &[String]) -> Option<TypeReference> {
        let (prefix, imported_path) = self.imported_modules.get(&segments[0])?;
        let mut full_segments = imported_path.clone();
        full_segments.extend(segments[1..].iter().cloned());

        Some(TypeReference::new(full_segments).with_prefix(*prefix))
    }

    /// Extracts internal crate path references from an opaque [`TokenStream`].
    ///
    /// Scans for `Ident :: Ident [:: Ident]*` patterns — sequences of identifiers
    /// separated by `::` punctuation. Each collected path is checked against the
    /// internal-path filter; external paths (e.g. `std::fmt`) are discarded.
    ///
    /// Recursively descends into grouped tokens (`()`, `[]`, `{}`) to find nested
    /// paths (e.g. inside `vec![crate::foo::bar()]`).
    ///
    /// This enables dependency detection inside macro arguments and attribute values
    /// where `syn::Visit` does not traverse (token streams are opaque to the visitor).
    fn extract_paths_from_tokens(&mut self, tokens: &TokenStream) {
        let mut iter = tokens.clone().into_iter().peekable();

        while let Some(token) = iter.next() {
            match token {
                TokenTree::Ident(ref ident) => {
                    let mut segments = vec![ident.to_string()];

                    // Collect :: Ident sequences
                    loop {
                        // Check for first ':' with Joint spacing (start of ::)
                        let is_path_sep = matches!(
                            iter.peek(),
                            Some(TokenTree::Punct(p)) if p.as_char() == ':' && p.spacing() == Spacing::Joint
                        );
                        if !is_path_sep {
                            break;
                        }
                        iter.next(); // consume first ':'
                        iter.next(); // consume second ':'

                        // Expect Ident after ::
                        if let Some(TokenTree::Ident(next_ident)) = iter.peek() {
                            segments.push(next_ident.to_string());
                            iter.next(); // consume ident
                        } else {
                            break;
                        }
                    }

                    if segments.len() >= 2 {
                        if let Some(r) = self
                            .build_reference_from_segments(&segments)
                            .or_else(|| self.resolve_via_import(&segments))
                        {
                            self.references.value_refs.push(r);
                        }
                    }
                }
                TokenTree::Group(group) => {
                    self.extract_paths_from_tokens(&group.stream());
                }
                _ => {}
            }
        }
    }

    /// Recursively walks a `use` tree and pushes resolved [`TypeReference`]s into
    /// `self.references.use_statements`.
    ///
    /// `prefix` accumulates path segments seen so far; `path_prefix` tracks any
    /// leading keyword (`crate`, `self`, `super`) encountered during traversal.
    fn process_use_tree(&mut self, tree: &UseTree, prefix: Vec<String>, path_prefix: PathPrefix) {
        match tree {
            UseTree::Path(p) => {
                let ident = p.ident.to_string();

                // Check for special prefixes at the start
                let (new_prefix, new_path_prefix) = if prefix.is_empty() {
                    match ident.as_str() {
                        PATH_QUALIFIER_CRATE => (Vec::new(), PathPrefix::Crate),
                        PATH_QUALIFIER_SELF => (Vec::new(), PathPrefix::SelfModule),
                        PATH_QUALIFIER_SUPER => {
                            let levels = match path_prefix {
                                PathPrefix::Super(n) => n + 1,
                                _ => 1,
                            };
                            (Vec::new(), PathPrefix::Super(levels))
                        }
                        _ => {
                            let mut new_prefix = prefix;
                            new_prefix.push(ident);
                            (new_prefix, path_prefix)
                        }
                    }
                } else if ident == PATH_QUALIFIER_SUPER {
                    // Handle chained super:: in the middle of path
                    let levels = match path_prefix {
                        PathPrefix::Super(n) => n + 1,
                        _ => 1,
                    };
                    (prefix, PathPrefix::Super(levels))
                } else {
                    let mut new_prefix = prefix;
                    new_prefix.push(ident);
                    (new_prefix, path_prefix)
                };

                self.process_use_tree(&p.tree, new_prefix, new_path_prefix);
            }

            UseTree::Name(n) => {
                let imported_name = n.ident.to_string();
                let mut segments = prefix;
                segments.push(imported_name.clone());

                self.try_record_import(imported_name, &segments, path_prefix);

                let reference = resolve_reference(
                    TypeReference::new(segments).with_prefix(path_prefix),
                    &self.module_path,
                );
                self.references.use_statements.push(reference);
            }

            UseTree::Rename(r) => {
                let alias = r.rename.to_string();
                let mut segments = prefix;
                segments.push(r.ident.to_string());

                self.try_record_import(alias.clone(), &segments, path_prefix);

                let reference = resolve_reference(
                    TypeReference::new(segments)
                        .with_prefix(path_prefix)
                        .with_alias(alias),
                    &self.module_path,
                );
                self.references.use_statements.push(reference);
            }

            UseTree::Glob(_) => {
                let reference = resolve_reference(
                    TypeReference::new(prefix)
                        .with_prefix(path_prefix)
                        .with_glob(),
                    &self.module_path,
                );
                self.references.use_statements.push(reference);
            }

            UseTree::Group(g) => {
                // Record individual imports from group items
                for item in &g.items {
                    match item {
                        UseTree::Name(n) => {
                            let name = n.ident.to_string();
                            let mut item_segments = prefix.clone();
                            item_segments.push(name.clone());
                            self.try_record_import(name, &item_segments, path_prefix);
                        }
                        UseTree::Rename(r) => {
                            let alias = r.rename.to_string();
                            let mut item_segments = prefix.clone();
                            item_segments.push(r.ident.to_string());
                            self.try_record_import(alias, &item_segments, path_prefix);
                        }
                        _ => {}
                    }
                }

                let group_items = self.convert_group(&g.items);

                let reference = resolve_reference(
                    TypeReference::new(prefix)
                        .with_prefix(path_prefix)
                        .with_group(group_items),
                    &self.module_path,
                );
                self.references.use_statements.push(reference);
            }
        }
    }

    fn convert_group(
        &self,
        items: &syn::punctuated::Punctuated<UseTree, syn::token::Comma>,
    ) -> Vec<GroupItem> {
        items
            .iter()
            .map(|item| self.convert_use_tree(item))
            .collect()
    }

    fn convert_use_tree(&self, tree: &UseTree) -> GroupItem {
        match tree {
            UseTree::Name(n) => {
                let ident = n.ident.to_string();
                if ident == PATH_QUALIFIER_SELF {
                    GroupItem::SelfItem { alias: None }
                } else {
                    GroupItem::Simple(ident)
                }
            }

            UseTree::Rename(r) => {
                let ident = r.ident.to_string();
                let alias = r.rename.to_string();
                if ident == PATH_QUALIFIER_SELF {
                    GroupItem::SelfItem { alias: Some(alias) }
                } else {
                    GroupItem::Aliased { name: ident, alias }
                }
            }

            UseTree::Glob(_) => GroupItem::Glob,

            UseTree::Path(p) => {
                let mut prefix = vec![p.ident.to_string()];
                let mut current = &*p.tree;

                // Flatten nested paths
                while let UseTree::Path(inner) = current {
                    prefix.push(inner.ident.to_string());
                    current = &*inner.tree;
                }

                match current {
                    UseTree::Group(g) => GroupItem::Nested {
                        prefix,
                        items: self.convert_group(&g.items),
                    },
                    UseTree::Name(n) => GroupItem::Nested {
                        prefix,
                        items: vec![GroupItem::Simple(n.ident.to_string())],
                    },
                    UseTree::Rename(r) => {
                        prefix.push(r.ident.to_string());
                        GroupItem::Nested {
                            prefix: prefix[..prefix.len() - 1].to_vec(),
                            items: vec![GroupItem::Aliased {
                                name: r.ident.to_string(),
                                alias: r.rename.to_string(),
                            }],
                        }
                    }
                    UseTree::Glob(_) => GroupItem::Nested {
                        prefix,
                        items: vec![GroupItem::Glob],
                    },
                    UseTree::Path(_) => GroupItem::Nested {
                        prefix,
                        items: Vec::new(),
                    },
                }
            }

            UseTree::Group(g) => GroupItem::Nested {
                prefix: Vec::new(),
                items: self.convert_group(&g.items),
            },
        }
    }
}

impl<'ast> Visit<'ast> for ModuleVisitor {
    /// Visit attributes - extracts internal paths from attribute arguments.
    ///
    /// Handles `#[command(version = version::VERSION)]` and similar patterns
    /// where paths appear inside attribute token streams that `syn::Visit`
    /// normally treats as opaque.
    fn visit_attribute(&mut self, attr: &'ast syn::Attribute) {
        if !self.in_test_module {
            if let syn::Meta::List(meta_list) = &attr.meta {
                self.extract_paths_from_tokens(&meta_list.tokens);
            }
        }
        syn::visit::visit_attribute(self, attr);
    }

    /// Visit expression paths - captures paths in expressions like `crate::foo::bar()`
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.value_refs.push(r);
            }
        }
        syn::visit::visit_expr_path(self, node);
    }

    /// Visit struct expressions - captures struct literal construction
    fn visit_expr_struct(&mut self, node: &'ast syn::ExprStruct) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.value_refs.push(r);
            }
        }
        syn::visit::visit_expr_struct(self, node);
    }

    /// Visit impl items - captures impl blocks
    fn visit_item_impl(&mut self, node: &'ast syn::ItemImpl) {
        if !self.in_test_module {
            // Check the trait being implemented (if any)
            if let Some((_, trait_path, _)) = &node.trait_ {
                if let Some(r) = self.build_reference(trait_path) {
                    self.references.type_refs.push(r);
                }
            }
        }
        syn::visit::visit_item_impl(self, node);
    }

    fn visit_item_mod(&mut self, i: &'ast ItemMod) {
        let was_in_test = self.in_test_module;

        // Check if this module is a test module (has #[cfg(test)] attribute)
        if has_cfg_test(&i.attrs) {
            self.in_test_module = true;
        }

        // Check if this module matches the target module or is on the path to it
        let module_ident = i.ident.to_string();
        let should_visit = if self.module_name.is_empty() {
            // No filter, visit all modules
            true
        } else if self.module_name == module_ident {
            // Exact match
            true
        } else if self.module_name.contains("::") {
            // For nested modules like "foo::bar::baz", check if this module
            // is on the path (e.g., ident is "foo" and module_name starts with "foo::")
            // or if this module is the final segment
            self.module_name.split("::").any(|seg| seg == module_ident)
                || self.module_name.ends_with(&format!("::{module_ident}"))
        } else {
            // Single-segment module name, check exact match
            self.module_name == module_ident
        };

        if !should_visit {
            debug!(
                "Skipping module '{module_ident}' (filter: '{}')",
                self.module_name
            );
        }
        if should_visit && let Some((_, items)) = &i.content {
            for item in items {
                self.visit_item(item);
            }
        }

        // Restore previous test module state
        self.in_test_module = was_in_test;
    }

    fn visit_item_use(&mut self, node: &'ast ItemUse) {
        if !self.in_test_module {
            self.process_use_tree(&node.tree, Vec::new(), PathPrefix::None);
        }
    }

    /// Visit macro invocations - captures macro paths and argument paths.
    ///
    /// In addition to the macro's own path, scans the macro's token stream
    /// for internal crate paths (e.g. `info!("...", version::NAME)`).
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.macro_calls.push(r);
            }
            self.extract_paths_from_tokens(&node.tokens);
        }
        syn::visit::visit_macro(self, node);
    }

    /// Visit pattern structs - captures struct patterns in match arms
    fn visit_pat_struct(&mut self, node: &'ast syn::PatStruct) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.value_refs.push(r);
            }
        }
        syn::visit::visit_pat_struct(self, node);
    }

    /// Visit pattern tuple structs - captures tuple struct patterns
    fn visit_pat_tuple_struct(&mut self, node: &'ast syn::PatTupleStruct) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.value_refs.push(r);
            }
        }
        syn::visit::visit_pat_tuple_struct(self, node);
    }

    /// Visit trait bounds - captures trait bounds in generics
    fn visit_trait_bound(&mut self, node: &'ast syn::TraitBound) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.type_refs.push(r);
            }
        }
        syn::visit::visit_trait_bound(self, node);
    }

    /// Visit type paths - captures type annotations like `let x: crate::Foo`
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if !self.in_test_module {
            if let Some(r) = self.build_reference(&node.path) {
                self.references.type_refs.push(r);
            }

            // Also check the qself if present (e.g., <crate::Foo as Trait>::Item)
            if let Some(qself) = &node.qself {
                syn::visit::visit_type(self, &qself.ty);
            }
        }
        syn::visit::visit_type_path(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_self_prefix() {
        let r = TypeReference::new(["foo", "Bar"]).with_self_prefix();
        let module_path = vec!["utils".to_owned(), "parser".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Crate);
        assert_eq!(resolved.segments(), &["utils", "parser", "foo", "Bar"]);
        assert_eq!(resolved.to_path_string(), "crate::utils::parser::foo::Bar");
    }

    #[test]
    fn test_resolve_self_prefix_at_crate_root() {
        let r = TypeReference::new(["foo", "Bar"]).with_self_prefix();
        let module_path: Vec<String> = vec![];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Crate);
        assert_eq!(resolved.segments(), &["foo", "Bar"]);
        assert_eq!(resolved.to_path_string(), "crate::foo::Bar");
    }

    #[test]
    fn test_resolve_super_single_level() {
        let r = TypeReference::new(["sibling", "Type"]).with_super(1);
        let module_path = vec!["parent".to_owned(), "child".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Crate);
        assert_eq!(resolved.segments(), &["parent", "sibling", "Type"]);
        assert_eq!(resolved.to_path_string(), "crate::parent::sibling::Type");
    }

    #[test]
    fn test_resolve_super_multiple_levels() {
        let r = TypeReference::new(["ancestor", "Type"]).with_super(2);
        let module_path = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Crate);
        assert_eq!(resolved.segments(), &["a", "ancestor", "Type"]);
        assert_eq!(resolved.to_path_string(), "crate::a::ancestor::Type");
    }

    #[test]
    fn test_resolve_super_at_crate_root() {
        let r = TypeReference::new(["foo", "Bar"]).with_super(1);
        let module_path: Vec<String> = vec![];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Super(1));
        assert_eq!(resolved.segments(), &["foo", "Bar"]);
    }

    #[test]
    fn test_resolve_super_too_many_levels() {
        let r = TypeReference::new(["foo", "Bar"]).with_super(5);
        let module_path = vec!["a".to_owned(), "b".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Super(5));
        assert_eq!(resolved.segments(), &["foo", "Bar"]);
    }

    #[test]
    fn test_resolve_crate_prefix_unchanged() {
        let r = TypeReference::new(["module", "Type"]).with_crate_prefix();
        let module_path = vec!["utils".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::Crate);
        assert_eq!(resolved.segments(), &["module", "Type"]);
        assert_eq!(resolved.to_path_string(), "crate::module::Type");
    }

    #[test]
    fn test_resolve_no_prefix_unchanged() {
        let r = TypeReference::new(["std", "collections", "HashMap"]);
        let module_path = vec!["utils".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.prefix(), PathPrefix::None);
        assert_eq!(resolved.segments(), &["std", "collections", "HashMap"]);
        assert_eq!(resolved.to_path_string(), "std::collections::HashMap");
    }

    #[test]
    fn test_resolve_preserves_suffix() {
        let r = TypeReference::new(["foo", "Bar"])
            .with_self_prefix()
            .with_alias("MyBar");
        let module_path = vec!["utils".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.to_path_string(), "crate::utils::foo::Bar as MyBar");
    }

    #[test]
    fn test_resolve_with_glob() {
        let r = TypeReference::new(["foo"]).with_self_prefix().with_glob();
        let module_path = vec!["utils".to_owned()];
        let resolved = resolve_reference(r, &module_path);

        assert_eq!(resolved.to_path_string(), "crate::utils::foo::*");
    }

    // --- Token stream path extraction tests ---

    fn visitor_with_children(children: &[&str]) -> ModuleVisitor {
        let children = children.iter().map(|s| (*s).to_owned()).collect();
        ModuleVisitor::new("test_mod", children, None)
    }

    fn tokens(code: &str) -> TokenStream {
        code.parse().expect("failed to parse token stream")
    }

    #[test]
    fn test_extract_crate_path_from_tokens() {
        let mut v = visitor_with_children(&[]);
        v.extract_paths_from_tokens(&tokens("foo, crate::version::VERSION, bar"));

        assert_eq!(v.references.value_refs.len(), 1);
        assert_eq!(
            v.references.value_refs[0].to_path_string(),
            "crate::version::VERSION"
        );
    }

    #[test]
    fn test_extract_child_module_path_from_tokens() {
        let mut v = visitor_with_children(&["version"]);
        v.extract_paths_from_tokens(&tokens("version::NAME"));

        assert_eq!(v.references.value_refs.len(), 1);
        assert_eq!(v.references.value_refs[0].to_path_string(), "version::NAME");
    }

    #[test]
    fn test_extract_skips_external_paths() {
        let mut v = visitor_with_children(&["version"]);
        v.extract_paths_from_tokens(&tokens("std::fmt::Display, tracing::info"));

        assert!(v.references.value_refs.is_empty());
    }

    #[test]
    fn test_extract_skips_lone_ident() {
        let mut v = visitor_with_children(&["version"]);
        v.extract_paths_from_tokens(&tokens("version"));

        assert!(v.references.value_refs.is_empty());
    }

    #[test]
    fn test_extract_paths_from_nested_groups() {
        let mut v = visitor_with_children(&[]);
        v.extract_paths_from_tokens(&tokens("(crate::foo::bar, (crate::baz::qux))"));

        assert_eq!(v.references.value_refs.len(), 2);
        assert_eq!(
            v.references.value_refs[0].to_path_string(),
            "crate::foo::bar"
        );
        assert_eq!(
            v.references.value_refs[1].to_path_string(),
            "crate::baz::qux"
        );
    }

    #[test]
    fn test_extract_self_path_from_tokens() {
        let mut v = visitor_with_children(&[]);
        v.extract_paths_from_tokens(&tokens("self::utils::helper"));

        assert_eq!(v.references.value_refs.len(), 1);
        assert_eq!(
            v.references.value_refs[0].to_path_string(),
            "crate::test_mod::utils::helper"
        );
    }

    #[test]
    fn test_extract_super_path_from_tokens() {
        let mut v = visitor_with_children(&[]);
        v.extract_paths_from_tokens(&tokens("super::sibling::Type"));

        assert_eq!(v.references.value_refs.len(), 1);
        // test_mod has 1 segment, super goes up 1 level
        assert_eq!(
            v.references.value_refs[0].to_path_string(),
            "crate::sibling::Type"
        );
    }

    #[test]
    fn test_extract_multiple_paths_from_tokens() {
        let mut v = visitor_with_children(&["version"]);
        v.extract_paths_from_tokens(&tokens("\"fmt\", version::NAME, version::VERSION"));

        assert_eq!(v.references.value_refs.len(), 2);
    }

    #[test]
    fn test_extract_preserves_duplicates() {
        let mut v = visitor_with_children(&["version"]);
        v.extract_paths_from_tokens(&tokens("version::NAME, version::NAME"));

        assert_eq!(v.references.value_refs.len(), 2);
    }

    // --- Import-aware token resolution tests ---

    fn visitor_with_package(package: &str) -> ModuleVisitor {
        ModuleVisitor::new("test_mod", HashSet::new(), Some(package.to_owned()))
    }

    #[test]
    fn test_import_resolution_via_package_name() {
        let code = r#"
            use myapp::version;
            fn foo() {
                info!("{}", version::NAME);
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = visitor_with_package("myapp");
        v.visit_file(&syntax);

        let value_paths: Vec<String> = v
            .references
            .value_refs
            .iter()
            .map(TypeReference::to_path_string)
            .collect();
        assert!(
            value_paths.contains(&"myapp::version::NAME".to_owned()),
            "Expected myapp::version::NAME in {value_paths:?}"
        );
    }

    #[test]
    fn test_import_resolution_via_crate_prefix() {
        let code = r#"
            use crate::utils;
            fn foo() {
                debug!("{}", utils::helper());
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = visitor_with_package("myapp");
        v.visit_file(&syntax);

        let value_paths: Vec<String> = v
            .references
            .value_refs
            .iter()
            .map(TypeReference::to_path_string)
            .collect();
        assert!(
            value_paths.contains(&"crate::utils::helper".to_owned()),
            "Expected crate::utils::helper in {value_paths:?}"
        );
    }

    #[test]
    fn test_import_resolution_with_alias() {
        let code = r#"
            use myapp::version as ver;
            fn foo() {
                println!("{}", ver::NAME);
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = visitor_with_package("myapp");
        v.visit_file(&syntax);

        let value_paths: Vec<String> = v
            .references
            .value_refs
            .iter()
            .map(TypeReference::to_path_string)
            .collect();
        assert!(
            value_paths.contains(&"myapp::version::NAME".to_owned()),
            "Expected myapp::version::NAME in {value_paths:?}"
        );
    }

    #[test]
    fn test_import_resolution_from_group() {
        let code = r#"
            use myapp::{version, model};
            fn foo() {
                info!("{}", version::NAME);
                debug!("{}", model::Type);
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = visitor_with_package("myapp");
        v.visit_file(&syntax);

        let value_paths: Vec<String> = v
            .references
            .value_refs
            .iter()
            .map(TypeReference::to_path_string)
            .collect();
        assert!(
            value_paths.contains(&"myapp::version::NAME".to_owned()),
            "Expected myapp::version::NAME in {value_paths:?}"
        );
        assert!(
            value_paths.contains(&"myapp::model::Type".to_owned()),
            "Expected myapp::model::Type in {value_paths:?}"
        );
    }

    #[test]
    fn test_import_resolution_skips_external() {
        let code = r#"
            use serde::Serialize;
            fn foo() {
                info!("{}", Serialize::something);
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = visitor_with_package("myapp");
        v.visit_file(&syntax);

        assert!(
            v.references.value_refs.is_empty(),
            "External imports should not resolve: {:?}",
            v.references
                .value_refs
                .iter()
                .map(TypeReference::to_path_string)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn test_import_resolution_no_package_name() {
        let code = r#"
            use myapp::version;
            fn foo() {
                info!("{}", version::NAME);
            }
        "#;
        let syntax: syn::File = syn::parse_file(code).expect("parse");
        let mut v = ModuleVisitor::new("test_mod", HashSet::new(), None);
        v.visit_file(&syntax);

        // Without package_name, version::NAME should not resolve
        assert!(
            v.references.value_refs.is_empty(),
            "Without package_name, should not resolve imported names"
        );
    }
}
