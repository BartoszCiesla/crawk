//! Module for analyzing type usage in Rust source files.
//!
//! Provides a single unified type [`TypeReference`] to represent all forms of
//! type/path references in Rust code: `use` statements, fully qualified paths,
//! and relative paths (`self`, `super`, `crate`).
use std::fmt::{Display, Formatter, Result};
use std::ops::{Deref, DerefMut};

/// Ordered list of path segments (e.g., `["std", "collections", "HashMap"]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Segments(Vec<String>);

impl Segments {
    /// Creates a new `Segments` from an iterator of string-like values.
    pub fn new<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(iter.into_iter().map(Into::into).collect())
    }

    /// Returns true if there are no segments.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of segments.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }
}

impl Deref for Segments {
    type Target = Vec<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Segments {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<S: Into<String>> FromIterator<S> for Segments {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        Self::new(iter)
    }
}

impl From<Vec<String>> for Segments {
    fn from(v: Vec<String>) -> Self {
        Self(v)
    }
}

/// Unified type reference representing any path/type usage in Rust code.
///
/// # Examples
///
/// ```ignore
/// use crawk::module::path::TypeReference;
///
/// // Simple use: `use std::collections::HashMap;`
/// let r = TypeReference::new(vec!["std", "collections", "HashMap"]);
///
/// // Glob: `use std::collections::*;`
/// let r = TypeReference::new(vec!["std", "collections"]).with_glob();
///
/// // Relative with self: `use self::module::Type;`
/// let r = TypeReference::new(vec!["module", "Type"]).with_self_prefix();
///
/// // Super: `use super::sibling::Type;`
/// let r = TypeReference::new(vec!["sibling", "Type"]).with_super(1);
///
/// // Crate root: `use crate::module::Type;`
/// let r = TypeReference::new(vec!["module", "Type"]).with_crate_prefix();
///
/// // Grouped: `use std::collections::{HashMap, HashSet};`
/// let r = TypeReference::new(vec!["std", "collections"])
///     .with_group(vec!["HashMap".into(), "HashSet".into()]);
///
/// // Aliased: `use std::collections::HashMap as Map;`
/// let r = TypeReference::new(vec!["std", "collections", "HashMap"])
///     .with_alias("Map");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeReference {
    /// Path segments (e.g., `["std", "collections", "HashMap"]`)
    pub segments: Segments,

    /// Path prefix type for relative resolution
    pub prefix: PathPrefix,

    /// Path suffix type (alias, glob, or group)
    pub suffix: PathSuffix,
}

/// Suffix determining how a path ends (alias, glob, or group).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum PathSuffix {
    /// No suffix - simple path
    #[default]
    None,

    /// Aliased import: `as Name`
    Alias(String),

    /// Glob import: `::*`
    Glob,

    /// Grouped import: `{A, B, C}`
    Group(Vec<GroupItem>),
}

/// Prefix determining how path is resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PathPrefix {
    /// No special prefix - external crate or prelude
    #[default]
    None,

    /// `crate::` - absolute from current crate root
    Crate,

    /// `self::` - relative to current module
    SelfModule,

    /// `super::` - relative to parent module(s)
    /// The value is how many `super` levels (1 = `super::`, 2 = `super::super::`, etc.)
    Super(usize),
}

/// Item within a grouped import `{...}`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GroupItem {
    /// Simple name: `HashMap`
    Simple(String),

    /// Aliased: `HashMap as Map`
    Aliased { name: String, alias: String },

    /// Self reference: `self` or `self as Alias`
    SelfItem { alias: Option<String> },

    /// Glob in group: `*`
    Glob,

    /// Nested group: `io::{Read, Write}`
    Nested {
        prefix: Vec<String>,
        items: Vec<Self>,
    },
}

impl TypeReference {
    /// Creates a new type reference from path segments.
    pub fn new<I, S>(segments: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            segments: Segments::new(segments),
            prefix: PathPrefix::None,
            suffix: PathSuffix::None,
        }
    }

    /// Sets `crate::` prefix.
    #[must_use]
    pub const fn with_crate_prefix(mut self) -> Self {
        self.prefix = PathPrefix::Crate;
        self
    }

    /// Sets `self::` prefix.
    #[must_use]
    pub const fn with_self_prefix(mut self) -> Self {
        self.prefix = PathPrefix::SelfModule;
        self
    }

    /// Sets `super::` prefix with given level count.
    #[must_use]
    pub const fn with_super(mut self, levels: usize) -> Self {
        self.prefix = PathPrefix::Super(levels);
        self
    }

    /// Sets an alias (`as Name`).
    #[must_use]
    pub fn with_alias(mut self, alias: impl Into<String>) -> Self {
        self.suffix = PathSuffix::Alias(alias.into());
        self
    }

    /// Marks as glob import (`::*`).
    #[must_use]
    pub fn with_glob(mut self) -> Self {
        self.suffix = PathSuffix::Glob;
        self
    }

    /// Sets grouped items (`{A, B, C}`).
    #[must_use]
    pub fn with_group(mut self, items: Vec<GroupItem>) -> Self {
        self.suffix = PathSuffix::Group(items);
        self
    }

    /// Returns true if this is a relative path (`self::`, `super::`, `crate::`).
    #[must_use]
    pub const fn is_relative(&self) -> bool {
        !matches!(self.prefix, PathPrefix::None)
    }

    /// Checks if this path is from a specific crate.
    #[must_use]
    pub fn is_from_crate(&self, crate_name: &str) -> bool {
        if self.prefix == PathPrefix::None
            && let Some(first_segment) = self.segments.first()
        {
            return first_segment == crate_name;
        }
        false
    }

    /// Returns true if this has a glob.
    #[must_use]
    pub const fn has_glob(&self) -> bool {
        matches!(self.suffix, PathSuffix::Glob)
    }

    /// Returns true if this has a group.
    #[must_use]
    pub const fn has_group(&self) -> bool {
        matches!(self.suffix, PathSuffix::Group(_))
    }

    /// Returns the final name (last segment or alias if present).
    #[must_use]
    pub fn final_name(&self) -> Option<&str> {
        match &self.suffix {
            PathSuffix::Alias(alias) => Some(alias.as_str()),
            _ => self.segments.last().map(String::as_str),
        }
    }

    fn append_segment(mut self, segment: impl Into<String>) -> Self {
        self.segments.push(segment.into());
        self
    }

    #[must_use]
    fn clone_with(&self, prefix: bool, suffix: bool) -> Self {
        Self {
            segments: self.segments.clone(),
            prefix: if prefix {
                self.prefix
            } else {
                PathPrefix::None
            },
            suffix: if suffix {
                self.suffix.clone()
            } else {
                PathSuffix::None
            },
        }
    }

    pub fn expand_suffix(&self) -> Vec<Self> {
        match &self.suffix {
            PathSuffix::None | PathSuffix::Alias(_) => vec![self.clone_with(true, false)],
            PathSuffix::Glob => vec![self.clone_with(true, true)],
            PathSuffix::Group(items) => {
                let mut result = Vec::new();
                for item in items {
                    match item {
                        GroupItem::Simple(name) | GroupItem::Aliased { name, alias: _ } => {
                            result.push(self.clone_with(true, false).append_segment(name));
                        }
                        GroupItem::SelfItem { alias: _ } => {
                            result.push(self.clone_with(true, false));
                        }
                        GroupItem::Glob => {
                            // Cannot expand glob without context, return as-is
                            result.push(self.clone_with(true, true));
                        }
                        GroupItem::Nested {
                            prefix,
                            items: nested_items,
                        } => {
                            let mut nested = self.clone_with(true, false);
                            nested.segments.extend(prefix.iter().cloned());
                            nested.suffix = PathSuffix::Group(nested_items.clone());
                            result.extend(nested.expand_suffix());
                        }
                    }
                }
                result
            }
        }
    }

    /// Converts to string representation.
    #[must_use]
    pub fn to_path_string(&self) -> String {
        let mut result = String::new();

        // Prefix
        match self.prefix {
            PathPrefix::None => {}
            PathPrefix::Crate => result.push_str("crate::"),
            PathPrefix::SelfModule => result.push_str("self::"),
            PathPrefix::Super(n) => {
                for _ in 0..n {
                    result.push_str("super::");
                }
            }
        }

        // Segments
        if !self.segments.is_empty() {
            result.push_str(&self.segments.join("::"));
        }

        // Suffix (group, glob, or alias)
        match &self.suffix {
            PathSuffix::None => {}
            PathSuffix::Group(group) => {
                if !self.segments.is_empty() {
                    result.push_str("::");
                }
                result.push('{');
                let items: Vec<_> = group.iter().map(ToString::to_string).collect();
                result.push_str(&items.join(", "));
                result.push('}');
            }
            PathSuffix::Glob => {
                result.push_str("::*");
            }
            PathSuffix::Alias(alias) => {
                result.push_str(" as ");
                result.push_str(alias);
            }
        }

        result
    }
}

impl Display for GroupItem {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Simple(name) => write!(f, "{name}"),
            Self::Aliased { name, alias } => write!(f, "{name} as {alias}"),
            Self::SelfItem { alias: None } => write!(f, "self"),
            Self::SelfItem { alias: Some(a) } => write!(f, "self as {a}"),
            Self::Glob => write!(f, "*"),
            Self::Nested { prefix, items } => {
                write!(f, "{}", prefix.join("::"))?;
                // Skip ::{} for empty items or single simple item
                if items.is_empty() {
                    Ok(())
                } else if items.len() == 1 {
                    if let Some(Self::Simple(name)) = items.first() {
                        write!(f, "::{name}")
                    } else {
                        write!(
                            f,
                            "::{{{}}}",
                            items
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    }
                } else {
                    let items_str: Vec<_> = items.iter().map(ToString::to_string).collect();
                    write!(f, "::{{{}}}", items_str.join(", "))
                }
            }
        }
    }
}

impl From<&str> for GroupItem {
    fn from(s: &str) -> Self {
        if s == "*" {
            Self::Glob
        } else if s == "self" {
            Self::SelfItem { alias: None }
        } else {
            Self::Simple(s.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_path() {
        let r = TypeReference::new(["std", "collections", "HashMap"]);
        assert_eq!(r.to_path_string(), "std::collections::HashMap");
        assert!(!r.is_relative());
        assert!(!r.has_glob());
        assert!(!r.has_group());
    }

    #[test]
    fn test_crate_prefix() {
        let r = TypeReference::new(["module", "Type"]).with_crate_prefix();
        assert_eq!(r.to_path_string(), "crate::module::Type");
        assert!(r.is_relative());
    }

    #[test]
    fn test_self_prefix() {
        let r = TypeReference::new(["submodule", "Type"]).with_self_prefix();
        assert_eq!(r.to_path_string(), "self::submodule::Type");
        assert!(r.is_relative());
    }

    #[test]
    fn test_super_prefix() {
        let r = TypeReference::new(["sibling", "Type"]).with_super(2);
        assert_eq!(r.to_path_string(), "super::super::sibling::Type");
        assert!(r.is_relative());
    }

    #[test]
    fn test_glob() {
        let r = TypeReference::new(["std", "collections"]).with_glob();
        assert_eq!(r.to_path_string(), "std::collections::*");
        assert!(r.has_glob());
    }

    #[test]
    fn test_alias() {
        let r = TypeReference::new(["std", "collections", "HashMap"]).with_alias("Map");
        assert_eq!(r.to_path_string(), "std::collections::HashMap as Map");
        assert_eq!(r.final_name(), Some("Map"));
    }

    #[test]
    fn test_group() {
        let r = TypeReference::new(["std", "collections"])
            .with_group(vec!["HashMap".into(), "HashSet".into()]);
        assert_eq!(r.to_path_string(), "std::collections::{HashMap, HashSet}");
        assert!(r.has_group());
    }

    #[test]
    fn test_group_with_alias() {
        let r = TypeReference::new(["std", "collections"]).with_group(vec![
            GroupItem::Simple("HashMap".into()),
            GroupItem::Aliased {
                name: "HashSet".into(),
                alias: "Set".into(),
            },
        ]);
        assert_eq!(
            r.to_path_string(),
            "std::collections::{HashMap, HashSet as Set}"
        );
    }

    #[test]
    fn test_group_with_self() {
        let r = TypeReference::new(["module"]).with_group(vec![
            GroupItem::SelfItem { alias: None },
            GroupItem::Simple("Type".into()),
        ]);
        assert_eq!(r.to_path_string(), "module::{self, Type}");
    }

    #[test]
    fn test_nested_group() {
        let r = TypeReference::new(["std"]).with_group(vec![GroupItem::Nested {
            prefix: vec!["collections".into()],
            items: vec!["HashMap".into(), "HashSet".into()],
        }]);
        assert_eq!(r.to_path_string(), "std::{collections::{HashMap, HashSet}}");
    }

    #[test]
    fn test_complex_nested_group() {
        // m::n::{a, b, c::{x, y}}
        let r = TypeReference::new(["m", "n"]).with_group(vec![
            GroupItem::Simple("a".into()),
            GroupItem::Simple("b".into()),
            GroupItem::Nested {
                prefix: vec!["c".into()],
                items: vec![GroupItem::Simple("x".into()), GroupItem::Simple("y".into())],
            },
        ]);
        assert_eq!(r.to_path_string(), "m::n::{a, b, c::{x, y}}");
        assert!(r.has_group());
        assert!(!r.is_relative());
    }

    #[test]
    fn test_nested() {
        let r = TypeReference::new(Vec::<&str>::new())
            .with_super(1)
            .with_group(vec![
                GroupItem::Simple("Id".into()),
                GroupItem::Simple("ItemDisplay".into()),
                GroupItem::Simple("ItemDisplaySettings".into()),
                GroupItem::Simple("OptionNameExt".into()),
                GroupItem::Simple("OptionToString".into()),
                GroupItem::Nested {
                    prefix: vec!["area".into()],
                    items: vec![GroupItem::Simple("Area".into())],
                },
                GroupItem::Nested {
                    prefix: vec!["display".into()],
                    items: vec![GroupItem::Simple("NameDisplay".into())],
                },
                GroupItem::Nested {
                    prefix: vec!["lifespan".into()],
                    items: vec![GroupItem::Simple("LifeSpan".into())],
                },
                GroupItem::Nested {
                    prefix: vec!["ratings".into()],
                    items: vec![
                        GroupItem::Simple("AllRatings".into()),
                        GroupItem::Simple("UserRating".into()),
                        GroupItem::Simple("get_rating".into()),
                    ],
                },
            ]);

        assert_eq!(
            r.to_path_string(),
            "super::{Id, ItemDisplay, ItemDisplaySettings, OptionNameExt, OptionToString, area::Area, display::NameDisplay, lifespan::LifeSpan, ratings::{AllRatings, UserRating, get_rating}}"
        );
        assert!(r.has_group());
        assert!(r.is_relative());
    }

    fn make_ref(segments: &[&str], suffix: PathSuffix) -> TypeReference {
        let mut r = TypeReference::new(segments.iter().copied());
        r.suffix = suffix;
        r
    }

    fn expand_segments(r: &TypeReference) -> Vec<Vec<String>> {
        r.expand_suffix()
            .into_iter()
            .map(|t| t.segments.0)
            .collect()
    }

    #[test]
    fn test_expand_suffix_none_passthrough() {
        let r = make_ref(&["std", "collections"], PathSuffix::None);
        assert_eq!(expand_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_suffix_alias_passthrough() {
        let r = make_ref(
            &["std", "collections", "HashMap"],
            PathSuffix::Alias("Map".into()),
        );
        // Alias doesn't change the segments, just passes through
        assert_eq!(
            expand_segments(&r),
            vec![vec!["std", "collections", "HashMap"]]
        );
    }

    #[test]
    fn test_expand_suffix_glob_passthrough() {
        let r = make_ref(&["std", "collections"], PathSuffix::Glob);
        assert_eq!(expand_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_suffix_group_simple() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![
                GroupItem::Simple("HashMap".into()),
                GroupItem::Simple("HashSet".into()),
            ]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![
                vec!["std", "collections", "HashMap"],
                vec!["std", "collections", "HashSet"],
            ]
        );
    }

    #[test]
    fn test_expand_suffix_group_aliased_uses_original_name() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![GroupItem::Aliased {
                name: "HashMap".into(),
                alias: "Map".into(),
            }]),
        );
        // Aliased items expand to the original name (alias is discarded in segments)
        assert_eq!(
            expand_segments(&r),
            vec![vec!["std", "collections", "HashMap"]]
        );
    }

    #[test]
    fn test_expand_suffix_group_self_item_no_alias() {
        let r = make_ref(
            &["std", "collections", "module"],
            PathSuffix::Group(vec![GroupItem::SelfItem { alias: None }]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![vec!["std", "collections", "module"]]
        );
    }

    #[test]
    fn test_expand_suffix_group_self_item_with_alias() {
        let r = make_ref(
            &["std", "collections", "module"],
            PathSuffix::Group(vec![GroupItem::SelfItem {
                alias: Some("Alias".into()),
            }]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![vec!["std", "collections", "module"]]
        );
    }

    #[test]
    fn test_expand_suffix_group_self_item_empty_base_no_alias() {
        // SelfItem without alias on empty base: nothing appended
        let r = make_ref(
            &[],
            PathSuffix::Group(vec![GroupItem::SelfItem { alias: None }]),
        );
        assert_eq!(expand_segments(&r), vec![vec![] as Vec<String>]);
    }

    #[test]
    fn test_expand_suffix_group_glob_returns_base() {
        let r = make_ref(
            &["std", "collections"],
            PathSuffix::Group(vec![GroupItem::Glob]),
        );
        // Glob inside group cannot be expanded, base segments returned as-is
        assert_eq!(expand_segments(&r), vec![vec!["std", "collections"]]);
    }

    #[test]
    fn test_expand_suffix_group_nested() {
        let r = make_ref(
            &["std"],
            PathSuffix::Group(vec![GroupItem::Nested {
                prefix: vec!["collections".into()],
                items: vec![
                    GroupItem::Simple("HashMap".into()),
                    GroupItem::Simple("HashSet".into()),
                ],
            }]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![
                vec!["std", "collections", "HashMap"],
                vec!["std", "collections", "HashSet"],
            ]
        );
    }

    #[test]
    fn test_expand_suffix_group_mixed() {
        // m::n::{a, b as B, c::{x, y}, *}
        let r = make_ref(
            &["m", "n"],
            PathSuffix::Group(vec![
                GroupItem::Simple("a".into()),
                GroupItem::Aliased {
                    name: "b".into(),
                    alias: "B".into(),
                },
                GroupItem::Nested {
                    prefix: vec!["c".into()],
                    items: vec![GroupItem::Simple("x".into()), GroupItem::Simple("y".into())],
                },
                GroupItem::Glob,
            ]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![
                vec!["m", "n", "a"],
                vec!["m", "n", "b"],
                vec!["m", "n", "c", "x"],
                vec!["m", "n", "c", "y"],
                vec!["m", "n"], // glob returns base
            ]
        );
    }

    #[test]
    fn test_expand_suffix_deeply_nested() {
        // a::{b::{c::{d,e,f}}}
        let r = make_ref(
            &["a"],
            PathSuffix::Group(vec![GroupItem::Nested {
                prefix: vec!["b".into()],
                items: vec![GroupItem::Nested {
                    prefix: vec!["c".into()],
                    items: vec![
                        GroupItem::Simple("d".into()),
                        GroupItem::Simple("e".into()),
                        GroupItem::Simple("f".into()),
                    ],
                }],
            }]),
        );
        assert_eq!(
            expand_segments(&r),
            vec![
                vec!["a", "b", "c", "d"],
                vec!["a", "b", "c", "e"],
                vec!["a", "b", "c", "f"],
            ]
        );
    }
}
