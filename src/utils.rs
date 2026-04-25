//! Shared utility functions used across multiple modules.

use proc_macro2::TokenTree;
use std::path::Path;
use syn::{Attribute, Item, Meta};
use thiserror::Error;

use crate::constants::{ATTR_CFG, MODULE_NAME_TEST};

/// Maximum source file size accepted by the analyzer (10 MiB).
pub(crate) const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

/// Error returned by [`read_source_file`].
#[derive(Debug, Error)]
pub(crate) enum ReadFileError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("file too large: {size} bytes (limit {limit} bytes)")]
    TooLarge { size: u64, limit: u64 },
}

/// Reads a source file, rejecting files larger than [`MAX_FILE_BYTES`].
pub(crate) fn read_source_file(path: &Path) -> Result<String, ReadFileError> {
    let size = std::fs::metadata(path)?.len();
    if size > MAX_FILE_BYTES {
        return Err(ReadFileError::TooLarge {
            size,
            limit: MAX_FILE_BYTES,
        });
    }
    Ok(std::fs::read_to_string(path)?)
}

/// Navigate the AST into nested inline modules by path segments.
///
/// Given `items` and a path like `["a", "b"]`, descends into `mod a { mod b { ... } }`
/// and returns the items of the innermost matched module.
///
/// Returns `None` if `path` is empty, any segment is not found,
/// or the matched module has no inline body (`mod foo;`).
pub(crate) fn descend_inline_module<'a, T: AsRef<str>>(
    items: &'a [Item],
    path: &[T],
) -> Option<&'a [Item]> {
    let (head, tail) = path.split_first()?;
    let head = head.as_ref();
    for item in items {
        if let Item::Mod(m) = item
            && m.ident == head
            && let Some((_, nested)) = &m.content
        {
            return if tail.is_empty() {
                Some(nested)
            } else {
                descend_inline_module(nested, tail)
            };
        }
    }
    None
}

/// Checks if a slice of attributes contains `#[cfg(test)]` or a compound form
/// that includes `test` (e.g., `#[cfg(all(test, ...))]`, `#[cfg(any(test, ...))]`).
///
/// Returns `false` for `#[cfg(not(test))]` — the `not(...)` group is skipped.
pub(crate) fn has_cfg_test(attrs: &[Attribute]) -> bool {
    fn stream_contains_test(stream: proc_macro2::TokenStream) -> bool {
        let tokens: Vec<TokenTree> = stream.into_iter().collect();
        let mut i = 0;
        while i < tokens.len() {
            match &tokens[i] {
                TokenTree::Ident(ident) if ident == MODULE_NAME_TEST => return true,
                TokenTree::Ident(ident) if ident == "not" => {
                    // Skip the `not(...)` group entirely
                    if matches!(tokens.get(i + 1), Some(TokenTree::Group(_))) {
                        i += 2;
                        continue;
                    }
                }
                TokenTree::Group(group) => {
                    if stream_contains_test(group.stream()) {
                        return true;
                    }
                }
                _ => {}
            }
            i += 1;
        }
        false
    }

    for attr in attrs {
        if let Meta::List(meta_list) = &attr.meta
            && meta_list.path.is_ident(ATTR_CFG)
            && stream_contains_test(meta_list.tokens.clone())
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_simple_cfg_test() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(test)])];
        assert!(has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_all_with_test() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(all(test, feature = "foo"))])];
        assert!(has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_any_with_test() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(any(test, doc))])];
        assert!(has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_test_among_multiple_attrs() {
        let attrs: Vec<Attribute> = vec![
            parse_quote!(#[derive(Debug)]),
            parse_quote!(#[cfg(test)]),
            parse_quote!(#[allow(unused)]),
        ];
        assert!(has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_not_test_returns_false() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(not(test))])];
        assert!(!has_cfg_test(&attrs));
    }

    #[test]
    fn test_no_cfg_attr_returns_false() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[derive(Debug)])];
        assert!(!has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_other_target_returns_false() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(target_os = "linux")])];
        assert!(!has_cfg_test(&attrs));
    }

    #[test]
    fn test_empty_attrs_returns_false() {
        assert!(!has_cfg_test(&[]));
    }

    #[test]
    fn test_cfg_all_with_not_test_returns_false() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(all(not(test)))])];
        assert!(!has_cfg_test(&attrs));
    }

    #[test]
    fn test_cfg_all_test_and_not_test() {
        let attrs: Vec<Attribute> = vec![parse_quote!(#[cfg(all(test, not(feature = "x")))])];
        assert!(has_cfg_test(&attrs));
    }

    // --- descend_inline_module ---

    fn inline_mod(name: &str, items: &[Item]) -> Item {
        let ident: syn::Ident = syn::parse_str(name).expect("valid ident");
        Item::Mod(syn::parse_quote! { mod #ident { #(#items)* } })
    }

    fn struct_item(name: &str) -> Item {
        let ident: syn::Ident = syn::parse_str(name).expect("valid ident");
        syn::parse_quote! { struct #ident; }
    }

    #[test]
    fn descend_inline_module_finds_single_level() {
        let inner = struct_item("Foo");
        let items = vec![inline_mod("utils", &[inner])];
        let result = descend_inline_module(&items, &["utils"]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn descend_inline_module_finds_two_levels() {
        let leaf = struct_item("Leaf");
        let inner = inline_mod("inner", &[leaf]);
        let items = vec![inline_mod("outer", &[inner])];
        let result = descend_inline_module(&items, &["outer", "inner"]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);
    }

    #[test]
    fn descend_inline_module_empty_path_returns_none() {
        let items = vec![struct_item("Foo")];
        assert!(descend_inline_module(&items, &[] as &[&str]).is_none());
    }

    #[test]
    fn descend_inline_module_missing_segment_returns_none() {
        let items = vec![inline_mod("present", &[])];
        assert!(descend_inline_module(&items, &["absent"]).is_none());
    }

    #[test]
    fn descend_inline_module_no_body_returns_none() {
        // `mod foo;` — declaration without inline body
        let items: Vec<syn::Item> = vec![syn::parse_quote! { mod foo; }];
        assert!(descend_inline_module(&items, &["foo"]).is_none());
    }

    #[test]
    fn descend_inline_module_works_with_owned_strings() {
        let items = vec![inline_mod("utils", &[struct_item("Bar")])];
        let path: Vec<String> = vec!["utils".to_owned()];
        assert!(descend_inline_module(&items, &path).is_some());
    }

    // --- read_source_file ---

    #[test]
    fn read_source_file_rejects_file_exceeding_limit() {
        let file = tempfile::NamedTempFile::new().expect("create temp file");
        file.as_file()
            .set_len(MAX_FILE_BYTES + 1)
            .expect("set file length");
        let err = read_source_file(file.path()).unwrap_err();
        assert!(matches!(
            err,
            ReadFileError::TooLarge { size, limit }
                if size == MAX_FILE_BYTES + 1 && limit == MAX_FILE_BYTES
        ));
    }
}
