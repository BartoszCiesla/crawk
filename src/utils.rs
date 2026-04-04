//! Shared utility functions used across multiple modules.

use proc_macro2::TokenTree;
use syn::{Attribute, Meta};

use crate::constants::{ATTR_CFG, MODULE_NAME_TEST};

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
}
