//! Resolve glob imports by extracting the public API from a module's source file.
//!
//! Given a path to a `.rs` file, [`extract_public_items`] returns the names of
//! all publicly visible items (structs, enums, functions, constants, traits,
//! type aliases, modules, statics, and `pub use` re-exports).
//!
//! [`resolve_glob`] resolves a glob `TypeReference` (e.g., `crate::foo::bar::*`)
//! into concrete references by reading the target module's public API.
//!
//! ## Visibility handling
//!
//! Items are included if they are visible to the module performing the
//! glob-import (`caller_module`). Supported visibility forms:
//!
//! | Form | Behaviour |
//! |---|---|
//! | `pub` | always visible |
//! | `pub(crate)` | visible to all modules in the crate |
//! | `pub(super)` | visible in the parent module and its descendants |
//! | `pub(in path)` | visible to modules within the named path subtree |
//! | `pub(self)` / private | not visible |

use crate::cache::ParseCache;
use crate::discover::CrateInfo;
use crate::reference::{PathPrefix, TypeReference};
use crate::utils::read_source_file;
use std::path::Path;
use std::rc::Rc;
use syn::{Item, UseTree};
use tracing::debug;

/// Extract all public item names from a Rust source file.
///
/// Returns `None` if the file cannot be read or parsed.
/// Returns `Some(vec)` with the names of every item visible to `caller_module`
/// declared in `target_module`. For `pub use` re-exports, the *imported* name
/// (or alias) is included.
///
/// Visibility is resolved against `target_module` (the module being inspected)
/// and `caller_module` (the module performing the glob-import). Both use the
/// crawk-internal path format: segments joined by `::`, empty string for the
/// crate root, no `crate::` prefix.
///
/// When `inline_module` is non-empty, descends into the named inline module(s)
/// before extracting items. For example, `&["constants"]` extracts only from
/// `pub mod constants { ... }` within the file. Note that `target_module` must
/// already contain the full path including any inline segments — this function
/// does not adjust it while descending.
#[must_use]
pub(crate) fn extract_public_items(
    file_path: &Path,
    inline_module: &[&str],
    target_module: &str,
    caller_module: &str,
    cache: &mut ParseCache,
) -> Option<Vec<String>> {
    // Can't use `cache.get_or_parse` here: this function returns `Option`,
    // but `get_or_parse` requires a `Result`-returning closure.
    let file = if let Some(cached) = cache.get(file_path) {
        cached
    } else {
        let content = read_source_file(file_path).ok()?;
        let parsed = syn::parse_file(&content).ok()?;
        let rc = Rc::new(parsed);
        cache.insert(file_path.to_path_buf(), Rc::clone(&rc));
        rc
    };

    let items = if inline_module.is_empty() {
        &file.items
    } else {
        return extract_from_inline_module(
            &file.items,
            inline_module,
            target_module,
            caller_module,
        );
    };

    Some(collect_public_items(items, target_module, caller_module))
}

/// Descend into nested inline modules and extract public items from the target.
fn extract_from_inline_module(
    items: &[Item],
    module_path: &[&str],
    target_module: &str,
    caller_module: &str,
) -> Option<Vec<String>> {
    if module_path.is_empty() {
        return Some(collect_public_items(items, target_module, caller_module));
    }

    let target = module_path[0];
    for item in items {
        if let Item::Mod(item_mod) = item
            && item_mod.ident == target
            && let Some((_, nested_items)) = &item_mod.content
        {
            return extract_from_inline_module(
                nested_items,
                &module_path[1..],
                target_module,
                caller_module,
            );
        }
    }

    // Inline module not found
    None
}

/// Extract visibility and ident from items that have a single exported name.
///
/// Returns `None` for items without a simple ident (`Item::Use`, `Item::Impl`,
/// `Item::Verbatim`, etc.) — those are handled separately or ignored.
const fn item_vis_and_ident(item: &Item) -> Option<(&syn::Visibility, &proc_macro2::Ident)> {
    match item {
        Item::Const(i) => Some((&i.vis, &i.ident)),
        Item::Enum(i) => Some((&i.vis, &i.ident)),
        Item::Fn(i) => Some((&i.vis, &i.sig.ident)),
        Item::ExternCrate(i) => Some((&i.vis, &i.ident)),
        Item::Mod(i) => Some((&i.vis, &i.ident)),
        Item::Static(i) => Some((&i.vis, &i.ident)),
        Item::Struct(i) => Some((&i.vis, &i.ident)),
        Item::Trait(i) => Some((&i.vis, &i.ident)),
        Item::TraitAlias(i) => Some((&i.vis, &i.ident)),
        Item::Type(i) => Some((&i.vis, &i.ident)),
        Item::Union(i) => Some((&i.vis, &i.ident)),
        _ => None,
    }
}

/// Returns `true` if the item is visible from `caller_module` when declared
/// inside `target_module`.
///
/// Both paths use the crawk-internal format: segments joined by `::`, empty
/// string for the crate root, no `crate::` prefix.
///
/// Handles `pub`, `pub(crate)`, `pub(super)`, and `pub(in some::path)`.
/// `pub(self)` is treated as private and returns `false`.
fn is_visible_from(vis: &syn::Visibility, target_module: &str, caller_module: &str) -> bool {
    match vis {
        syn::Visibility::Public(_) => true,
        syn::Visibility::Restricted(r) => {
            if r.path.is_ident("crate") {
                // `pub(crate)` — visible across the whole crate.
                true
            } else if r.path.is_ident("super") {
                // `pub(super)` — visible in the parent of `target_module`
                // and all of that parent's descendants.
                let parent = parent_module(target_module);
                is_in_subtree(caller_module, parent)
            } else if r.path.is_ident("self") {
                // `pub(self)` — semantically equivalent to private.
                false
            } else {
                // `pub(in some::path)` — normalize the restriction path to
                // crawk-internal format and check whether the caller lies
                // within its subtree.
                let ancestor = normalize_in_path(&r.path, target_module);
                is_in_subtree(caller_module, &ancestor)
            }
        }
        syn::Visibility::Inherited => false,
    }
}

/// Normalize a `pub(in path)` restriction path to crawk-internal format.
///
/// Converts the leading keyword of the syn path to an absolute module path
/// using `target_module` as the resolution context:
///
/// - `crate::foo::bar` → `"foo::bar"`
/// - `super` (from `foo::bar`) → `"foo"`
/// - `super::baz` (from `foo::bar`) → `"foo::baz"`
/// - `self` (from `foo::bar`) → `"foo::bar"` (caller must be in that subtree)
/// - bare `foo::bar` → `"foo::bar"` (treated as crate-root-relative)
///
/// The result is used as the `ancestor` argument to [`is_in_subtree`].
fn normalize_in_path(path: &syn::Path, target_module: &str) -> String {
    let mut segs = path.segments.iter().map(|s| s.ident.to_string());
    match segs.next().as_deref() {
        Some("crate") => segs.collect::<Vec<_>>().join("::"),
        Some("super") => {
            let parent = parent_module(target_module);
            let rest: Vec<_> = segs.collect();
            if rest.is_empty() {
                parent.to_owned()
            } else if parent.is_empty() {
                rest.join("::")
            } else {
                format!("{parent}::{}", rest.join("::"))
            }
        }
        Some("self") => {
            let rest: Vec<_> = segs.collect();
            if rest.is_empty() {
                target_module.to_owned()
            } else if target_module.is_empty() {
                rest.join("::")
            } else {
                format!("{target_module}::{}", rest.join("::"))
            }
        }
        Some(first) => {
            // Bare path — treat as crate-root-relative.
            let rest: Vec<_> = segs.collect();
            if rest.is_empty() {
                first.to_owned()
            } else {
                format!("{first}::{}", rest.join("::"))
            }
        }
        None => String::new(),
    }
}

/// Returns the parent module of `module` in crawk-internal path format.
///
/// - `"foo::bar"` → `"foo"`
/// - `"foo"`      → `""` (crate root)
/// - `""`         → `""` (root has no parent; `pub(super)` there is a Rust
///   compile error, so we never need to answer meaningfully)
fn parent_module(module: &str) -> &str {
    module.rsplit_once("::").map_or("", |(parent, _)| parent)
}

/// Returns `true` if `module` lies in the subtree rooted at `ancestor`
/// (including `ancestor` itself). Empty `ancestor` denotes the crate root,
/// which contains every module.
fn is_in_subtree(module: &str, ancestor: &str) -> bool {
    if ancestor.is_empty() {
        true
    } else {
        module == ancestor || module.starts_with(&format!("{ancestor}::"))
    }
}

/// Collect names of items visible to `caller_module` from a list of syn items
/// declared in `target_module`.
fn collect_public_items(items: &[Item], target_module: &str, caller_module: &str) -> Vec<String> {
    let mut public_items = Vec::new();

    for item in items {
        if let Item::Use(use_item) = item {
            if is_visible_from(&use_item.vis, target_module, caller_module) {
                extract_use_names(&use_item.tree, &mut public_items);
            }
        } else if let Some((vis, ident)) = item_vis_and_ident(item) {
            if is_visible_from(vis, target_module, caller_module) {
                public_items.push(ident.to_string());
            }
        }
    }

    public_items
}

/// Recursively extract imported names from a `UseTree`.
///
/// For `pub use foo::Bar` the name `Bar` is collected.
/// For `pub use foo::Bar as Baz` the alias `Baz` is collected.
/// For `pub use foo::{A, B}` both `A` and `B` are collected.
/// Glob re-exports (`pub use foo::*`) are skipped because they cannot be
/// resolved without additional context.
fn extract_use_names(tree: &UseTree, items: &mut Vec<String>) {
    match tree {
        UseTree::Name(name) => {
            items.push(name.ident.to_string());
        }
        UseTree::Rename(rename) => {
            items.push(rename.rename.to_string());
        }
        UseTree::Path(path) => {
            extract_use_names(&path.tree, items);
        }
        UseTree::Group(group) => {
            for item in &group.items {
                extract_use_names(item, items);
            }
        }
        UseTree::Glob(_) => {
            // Can't resolve nested globs without further context
        }
    }
}

/// Resolve a glob `TypeReference` (e.g., `crate::foo::bar::*`) into concrete
/// references by reading the target module's public API.
///
/// Only `crate::` prefixed globs are resolved. Other prefixes pass through
/// unchanged. If the module file cannot be found or parsed, the original
/// glob reference is returned with a warning.
///
/// `caller_module` is the crawk-internal path of the module performing the
/// glob-import (empty string for the crate root). It is used to decide
/// visibility of `pub(super)` items in the target module.
pub(crate) fn resolve_glob(
    reference: &TypeReference,
    caller_module: &str,
    crate_info: &CrateInfo,
    cache: &mut ParseCache,
) -> Vec<TypeReference> {
    // Determine the module path to resolve.
    // Accept both `crate::foo::bar::*` (PathPrefix::Crate, segments=["foo","bar"])
    // and `mycrate::foo::bar::*` (PathPrefix::None, first segment == crate name).
    let is_crate_prefix = reference.prefix() == PathPrefix::Crate;
    let is_crate_name_prefix = reference.prefix() == PathPrefix::None
        && reference
            .segments()
            .first()
            .is_some_and(|s| s == crate_info.root_package_name());

    if !is_crate_prefix && !is_crate_name_prefix {
        return vec![reference.clone()];
    }

    let module_path = reference.segments().join("::");
    if module_path.is_empty() {
        return vec![reference.clone()];
    }

    // Resolve module path to file
    let file_path = match crate_info.resolve_module_path_to_file(&module_path) {
        Ok(path) => path,
        Err(e) => {
            debug!(
                "Cannot resolve glob for '{}': {e}",
                reference.to_path_string()
            );
            return vec![reference.clone()];
        }
    };

    // Determine if the target is an inline module within the file.
    // If resolving a shorter prefix yields the same file, the remaining
    // segments are the inline module path.
    let inline_path = detect_inline_path(reference, &file_path, crate_info);
    let inline_refs: Vec<&str> = inline_path.iter().map(String::as_str).collect();

    // `module_path` is the full path (including any inline segments) of the
    // module whose glob we're resolving — it is also the target for visibility
    // checks. See `extract_public_items` docs for the path format.
    let Some(public_items) =
        extract_public_items(&file_path, &inline_refs, &module_path, caller_module, cache)
    else {
        debug!("Cannot parse '{}' for glob resolution", file_path.display());
        return vec![reference.clone()];
    };

    if public_items.is_empty() {
        return vec![];
    }

    // Build one TypeReference per public item
    public_items
        .into_iter()
        .map(|item| {
            let mut segments = reference.segments().to_vec();
            segments.push(item);
            TypeReference::new(segments).with_prefix(reference.prefix())
        })
        .collect()
}

/// Detect which trailing segments of a module path are inline modules
/// within the resolved file.
///
/// Compares progressive shorter prefixes of the module path against the
/// resolved file. Once a shorter prefix resolves to a *different* file
/// (or fails), the remaining segments are the inline module path.
fn detect_inline_path(
    reference: &TypeReference,
    resolved_file: &Path,
    crate_info: &CrateInfo,
) -> Vec<String> {
    let segments = reference.segments();

    // Walk from the full path backwards, peeling off one segment at a time.
    // The segments that resolve to the same file are "consumed by" the file;
    // any remainder must be inline modules.
    for split in (1..segments.len()).rev() {
        let prefix_path = segments[..split].join("::");
        match crate_info.resolve_module_path_to_file(&prefix_path) {
            Ok(ref parent_file) if parent_file == resolved_file => {
                // The shorter prefix still resolves to the same file,
                // so segments[split..] are inline module names.
                return segments[split..].to_vec();
            }
            _ => {
                // Different file or resolution failed — this prefix is
                // a different module, keep peeling.
            }
        }
    }

    // No inline path detected — the file directly corresponds to the module.
    vec![]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Wrapper around `extract_public_items` for tests that don't care about
    /// caller/target modules. Uses empty paths — visibility of `pub(super)`
    /// items declared in the file root collapses to "visible from anywhere"
    /// (super of the crate root = the crate root, which contains every module).
    fn extract(path: &Path, inline: &[&str]) -> Option<Vec<String>> {
        extract_public_items(path, inline, "", "", &mut ParseCache::new())
    }

    /// Wrapper for tests that need to control caller/target modules explicitly.
    fn extract_with(
        path: &Path,
        inline: &[&str],
        target: &str,
        caller: &str,
    ) -> Option<Vec<String>> {
        extract_public_items(path, inline, target, caller, &mut ParseCache::new())
    }

    #[test]
    fn extracts_all_public_item_kinds() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub struct PublicStruct;
pub(crate) struct CrateStruct;
struct PrivateStruct;
pub fn public_function() {{}}
pub(crate) fn crate_function() {{}}
fn private_function() {{}}
pub const PUBLIC_CONST: i32 = 42;
pub(crate) const CRATE_CONST: i32 = 43;
const PRIVATE_CONST: i32 = 42;
pub enum PublicEnum {{ A, B }}
pub(crate) enum CrateEnum {{ X, Y }}
enum PrivateEnum {{ X, Y }}
pub type PublicType = String;
pub(crate) type CrateType = String;
type PrivateType = String;
pub mod public_module {{}}
pub(crate) mod crate_module {{}}
mod private_module {{}}
pub trait PublicTrait {{}}
pub(crate) trait CrateTrait {{}}
trait PrivateTrait {{}}
pub use std::collections::HashMap;
pub(crate) use std::collections::BTreeMap;
"
        )
        .unwrap();

        let items = extract(f.path(), &[]).unwrap();

        // pub — visible
        assert!(items.contains(&"PublicStruct".to_owned()));
        assert!(items.contains(&"public_function".to_owned()));
        assert!(items.contains(&"PUBLIC_CONST".to_owned()));
        assert!(items.contains(&"PublicEnum".to_owned()));
        assert!(items.contains(&"PublicType".to_owned()));
        assert!(items.contains(&"public_module".to_owned()));
        assert!(items.contains(&"PublicTrait".to_owned()));
        assert!(items.contains(&"HashMap".to_owned()));

        // pub(crate) — visible across the crate
        assert!(items.contains(&"CrateStruct".to_owned()));
        assert!(items.contains(&"crate_function".to_owned()));
        assert!(items.contains(&"CRATE_CONST".to_owned()));
        assert!(items.contains(&"CrateEnum".to_owned()));
        assert!(items.contains(&"CrateType".to_owned()));
        assert!(items.contains(&"crate_module".to_owned()));
        assert!(items.contains(&"CrateTrait".to_owned()));
        assert!(items.contains(&"BTreeMap".to_owned()));

        // private — hidden
        assert!(!items.contains(&"PrivateStruct".to_owned()));
        assert!(!items.contains(&"private_function".to_owned()));
        assert!(!items.contains(&"PRIVATE_CONST".to_owned()));
        assert!(!items.contains(&"PrivateEnum".to_owned()));
        assert!(!items.contains(&"PrivateType".to_owned()));
        assert!(!items.contains(&"private_module".to_owned()));
        assert!(!items.contains(&"PrivateTrait".to_owned()));
    }

    #[test]
    fn extracts_pub_static() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub static PUB_STATIC: u32 = 1;
static PRIV_STATIC: u32 = 2;
"
        )
        .unwrap();

        let items = extract(f.path(), &[]).unwrap();
        assert!(items.contains(&"PUB_STATIC".to_owned()));
        assert!(!items.contains(&"PRIV_STATIC".to_owned()));
    }

    #[test]
    fn extracts_pub_use_rename() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub use std::collections::HashMap as Map;
"
        )
        .unwrap();

        let items = extract(f.path(), &[]).unwrap();
        assert!(items.contains(&"Map".to_owned()));
        assert!(!items.contains(&"HashMap".to_owned()));
    }

    #[test]
    fn extracts_pub_use_group() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub use std::collections::{{HashMap, HashSet}};
"
        )
        .unwrap();

        let items = extract(f.path(), &[]).unwrap();
        assert!(items.contains(&"HashMap".to_owned()));
        assert!(items.contains(&"HashSet".to_owned()));
    }

    #[test]
    fn returns_none_for_nonexistent_file() {
        let result = extract(Path::new("/nonexistent/file.rs"), &[]);
        assert!(result.is_none());
    }

    #[test]
    fn returns_empty_for_no_public_items() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
struct Private;
fn helper() {{}}
"
        )
        .unwrap();

        let items = extract(f.path(), &[]).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn extracts_items_from_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub fn top_level() {{}}

pub mod inner {{
    pub fn inner_func() {{}}
    pub struct InnerStruct;
    fn private_in_inner() {{}}
}}
"
        )
        .unwrap();

        // Extract from root — should see top_level and inner (the module)
        let root_items = extract(f.path(), &[]).unwrap();
        assert!(root_items.contains(&"top_level".to_owned()));
        assert!(root_items.contains(&"inner".to_owned()));

        // Extract from inline module "inner"
        let inner_items = extract(f.path(), &["inner"]).unwrap();
        assert!(inner_items.contains(&"inner_func".to_owned()));
        assert!(inner_items.contains(&"InnerStruct".to_owned()));
        assert!(!inner_items.contains(&"private_in_inner".to_owned()));
        assert!(!inner_items.contains(&"top_level".to_owned()));
    }

    #[test]
    fn extracts_items_from_nested_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub mod outer {{
    pub mod inner {{
        pub const DEEP: u32 = 42;
        pub fn deep_fn() {{}}
    }}
    pub fn outer_fn() {{}}
}}
"
        )
        .unwrap();

        let items = extract(f.path(), &["outer", "inner"]).unwrap();
        assert!(items.contains(&"DEEP".to_owned()));
        assert!(items.contains(&"deep_fn".to_owned()));
        assert!(!items.contains(&"outer_fn".to_owned()));
    }

    #[test]
    fn returns_none_for_missing_inline_module() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub fn top_level() {{}}
"
        )
        .unwrap();

        let result = extract(f.path(), &["nonexistent"]);
        assert!(result.is_none());
    }

    #[test]
    fn pub_super_visible_from_parent_and_siblings() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub mod bar {{
    pub(super) fn helper() {{}}
    pub fn public_fn() {{}}
}}
"
        )
        .unwrap();

        // Target: `foo::bar` (inline bar in a file representing `foo`).
        // Caller: `foo` — helper is visible (caller is the parent of target).
        let items = extract_with(f.path(), &["bar"], "foo::bar", "foo").unwrap();
        assert!(items.contains(&"helper".to_owned()));
        assert!(items.contains(&"public_fn".to_owned()));

        // Caller: `foo::other` — sibling of bar, inside the parent subtree.
        let items = extract_with(f.path(), &["bar"], "foo::bar", "foo::other").unwrap();
        assert!(items.contains(&"helper".to_owned()));

        // Caller: `foo::other::deep` — descendant of a sibling, still in
        // the parent subtree.
        let items = extract_with(f.path(), &["bar"], "foo::bar", "foo::other::deep").unwrap();
        assert!(items.contains(&"helper".to_owned()));

        // Caller: `baz` — outside `foo`, not in the parent subtree. helper
        // must be hidden while public_fn remains visible.
        let items = extract_with(f.path(), &["bar"], "foo::bar", "baz").unwrap();
        assert!(!items.contains(&"helper".to_owned()));
        assert!(items.contains(&"public_fn".to_owned()));

        // Caller: crate root — also outside `foo`.
        let items = extract_with(f.path(), &["bar"], "foo::bar", "").unwrap();
        assert!(!items.contains(&"helper".to_owned()));
    }

    #[test]
    fn pub_super_in_top_level_module_behaves_like_pub_crate() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(f, "pub(super) fn helper() {{}}").unwrap();

        // Target: `foo` (top-level). Parent = crate root ("") → every caller
        // is in the subtree, so helper is visible everywhere.
        for caller in ["", "bar", "baz::deep", "foo"] {
            let items = extract_with(f.path(), &[], "foo", caller).unwrap();
            assert!(items.contains(&"helper".to_owned()), "caller = {caller:?}");
        }
    }

    #[test]
    fn pub_in_path_visible_from_within_scope() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub(in crate::visibility) fn restricted() {{}}
pub fn public_fn() {{}}
"
        )
        .unwrap();

        // Caller is exactly the restricted scope — visible.
        let items = extract_with(f.path(), &[], "visibility", "visibility").unwrap();
        assert!(items.contains(&"restricted".to_owned()));
        assert!(items.contains(&"public_fn".to_owned()));

        // Caller is a sub-module of the restricted scope — also visible.
        let items = extract_with(f.path(), &[], "visibility", "visibility::inner").unwrap();
        assert!(items.contains(&"restricted".to_owned()));
    }

    #[test]
    fn pub_in_path_hidden_from_outside_scope() {
        let mut f = NamedTempFile::new().unwrap();
        writeln!(
            f,
            r"
pub(in crate::visibility) fn restricted() {{}}
pub fn public_fn() {{}}
"
        )
        .unwrap();

        // Caller is outside the restricted scope.
        let items = extract_with(f.path(), &[], "visibility", "other_mod").unwrap();
        assert!(!items.contains(&"restricted".to_owned()));
        assert!(items.contains(&"public_fn".to_owned()));

        // Crate root is also outside.
        let items = extract_with(f.path(), &[], "visibility", "").unwrap();
        assert!(!items.contains(&"restricted".to_owned()));
    }

    #[test]
    fn normalize_in_path_crate_prefix() {
        let path: syn::Path = syn::parse_str("crate::foo::bar").unwrap();
        assert_eq!(normalize_in_path(&path, "foo::bar"), "foo::bar");

        let path: syn::Path = syn::parse_str("crate::visibility").unwrap();
        assert_eq!(normalize_in_path(&path, "visibility::inner"), "visibility");

        // `crate` alone → crate root
        let path: syn::Path = syn::parse_str("crate").unwrap();
        assert_eq!(normalize_in_path(&path, "foo"), "");
    }

    #[test]
    fn normalize_in_path_super_prefix() {
        // `pub(in super)` from within `foo::bar` → parent is `foo`
        let path: syn::Path = syn::parse_str("super").unwrap();
        assert_eq!(normalize_in_path(&path, "foo::bar"), "foo");

        // `pub(in super::sibling)` from within `foo::bar` → `foo::sibling`
        let path: syn::Path = syn::parse_str("super::sibling").unwrap();
        assert_eq!(normalize_in_path(&path, "foo::bar"), "foo::sibling");

        // `pub(in super)` from top-level module → crate root
        let path: syn::Path = syn::parse_str("super").unwrap();
        assert_eq!(normalize_in_path(&path, "foo"), "");
    }

    #[test]
    fn normalize_in_path_self_prefix() {
        let path: syn::Path = syn::parse_str("self").unwrap();
        assert_eq!(normalize_in_path(&path, "foo::bar"), "foo::bar");

        let path: syn::Path = syn::parse_str("self::inner").unwrap();
        assert_eq!(normalize_in_path(&path, "foo::bar"), "foo::bar::inner");
    }

    #[test]
    fn parent_module_handles_root_and_nested() {
        assert_eq!(parent_module("foo::bar"), "foo");
        assert_eq!(parent_module("foo::bar::baz"), "foo::bar");
        assert_eq!(parent_module("foo"), "");
        assert_eq!(parent_module(""), "");
    }

    #[test]
    fn is_in_subtree_semantics() {
        // Empty ancestor = crate root: every module is in its subtree.
        assert!(is_in_subtree("", ""));
        assert!(is_in_subtree("foo", ""));
        assert!(is_in_subtree("foo::bar", ""));

        // Exact match and descendants are in the subtree.
        assert!(is_in_subtree("foo", "foo"));
        assert!(is_in_subtree("foo::bar", "foo"));
        assert!(is_in_subtree("foo::bar::baz", "foo"));

        // Prefix-only matches are NOT enough (no `::` boundary).
        assert!(!is_in_subtree("foobar", "foo"));
        assert!(!is_in_subtree("baz", "foo"));
        assert!(!is_in_subtree("", "foo"));
    }
}
