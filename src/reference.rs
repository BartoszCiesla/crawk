//! Module for analyzing type usage in Rust source files.
//!
//! Provides a single unified type [`TypeReference`] to represent all forms of
//! type/path references in Rust code: `use` statements, fully qualified paths,
//! and relative paths (`self`, `super`, `crate`).
use std::fmt::{Display, Formatter, Result};
use std::ops::Deref;

use crate::constants::PATH_QUALIFIER_SELF;

/// Ordered list of path segments (e.g., `["std", "collections", "HashMap"]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub(crate) struct Segments(Vec<String>);

impl Segments {
    /// Creates a new `Segments` from an iterator of string-like values.
    pub(crate) fn new<I, S>(iter: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self(iter.into_iter().map(Into::into).collect())
    }

    /// Returns true if there are no segments.
    #[must_use]
    pub(crate) const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the number of segments.
    #[must_use]
    pub(crate) const fn len(&self) -> usize {
        self.0.len()
    }
}

impl Deref for Segments {
    type Target = [String];

    fn deref(&self) -> &Self::Target {
        &self.0
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

impl Display for Segments {
    /// Formats segments as a `::` separated path string (e.g., `std::collections::HashMap`).
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.0.join("::"))
    }
}

impl Display for TypeReference {
    /// Formats the full path string, including prefix (`crate::`, `self::`, etc.),
    /// segments, and suffix (alias, glob, or group).
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        write!(f, "{}", self.to_path_string())
    }
}

/// Unified type reference representing any path/type usage in Rust code.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TypeReference {
    /// Path segments (e.g., `["std", "collections", "HashMap"]`)
    segments: Segments,

    /// Path prefix type for relative resolution
    prefix: PathPrefix,

    /// Path suffix type (alias, glob, or group)
    suffix: PathSuffix,
}

/// Suffix determining how a path ends (alias, glob, or group).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
#[non_exhaustive]
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
#[non_exhaustive]
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
#[non_exhaustive]
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
    /// Creates a new [`TypeReference`] from path segments with no prefix or suffix.
    ///
    /// Use the builder methods ([`with_crate_prefix`](Self::with_crate_prefix),
    /// [`with_alias`](Self::with_alias), [`with_glob`](Self::with_glob), etc.)
    /// to set a prefix or suffix after construction.
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

    /// Returns the path segments.
    #[must_use]
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Returns the path prefix.
    #[must_use]
    pub const fn prefix(&self) -> PathPrefix {
        self.prefix
    }

    /// Sets the path prefix.
    #[must_use]
    pub(crate) const fn with_prefix(mut self, prefix: PathPrefix) -> Self {
        self.prefix = prefix;
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

    /// Appends a single segment to the end of the path.
    pub(crate) fn append_segment(mut self, segment: impl Into<String>) -> Self {
        self.segments.0.push(segment.into());
        self
    }

    /// Clones this reference, optionally preserving the prefix and/or suffix.
    ///
    /// When `prefix` is `false`, the clone's prefix is reset to [`PathPrefix::None`].
    /// When `suffix` is `false`, the clone's suffix is reset to [`PathSuffix::None`].
    #[must_use]
    pub(crate) fn clone_with(&self, prefix: bool, suffix: bool) -> Self {
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

    /// Returns the path suffix.
    #[must_use]
    pub(crate) const fn suffix(&self) -> &PathSuffix {
        &self.suffix
    }

    /// Sets segments and prefix directly (used by resolve logic).
    #[must_use]
    pub(crate) fn with_segments_and_prefix(
        mut self,
        segments: Vec<String>,
        prefix: PathPrefix,
    ) -> Self {
        self.segments = Segments::from(segments);
        self.prefix = prefix;
        self
    }

    /// Truncates the path to the given depth (number of segments).
    ///
    /// If the path has more segments than `depth`, the segments are truncated
    /// and the suffix is dropped. Otherwise, the reference is returned unchanged.
    ///
    /// # Examples
    ///
    /// ```
    /// use crawk::TypeReference;
    ///
    /// // "module::analyzer::Error" (3 segments) truncated to depth 2
    /// // becomes "module::analyzer" (suffix dropped)
    /// let reference = TypeReference::new(["module", "analyzer", "Error"]);
    /// let truncated = reference.truncate_to_depth(2);
    /// assert_eq!(truncated.to_path_string(), "module::analyzer");
    /// ```
    #[must_use]
    pub fn truncate_to_depth(&self, depth: usize) -> Self {
        if self.segments.len() <= depth {
            return self.clone();
        }
        Self {
            segments: Segments::from(self.segments[..depth].to_vec()),
            prefix: self.prefix,
            suffix: PathSuffix::None,
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
            result.push_str(&self.segments.to_string());
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
    /// Formats a group item as it would appear in Rust source code.
    ///
    /// For example: `HashMap`, `HashMap as Map`, `self`, `*`, or `io::{Read, Write}`.
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self {
            Self::Simple(name) => write!(f, "{name}"),
            Self::Aliased { name, alias } => write!(f, "{name} as {alias}"),
            Self::SelfItem { alias: None } => write!(f, "{PATH_QUALIFIER_SELF}"),
            Self::SelfItem { alias: Some(a) } => write!(f, "{PATH_QUALIFIER_SELF} as {a}"),
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
        } else if s == PATH_QUALIFIER_SELF {
            Self::SelfItem { alias: None }
        } else {
            Self::Simple(s.to_owned())
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

    #[test]
    fn truncate_to_depth_zero_yields_empty_segments() {
        let r = TypeReference::new(["a", "b", "c"]).with_crate_prefix();
        let t = r.truncate_to_depth(0);
        assert_eq!(t.segments().len(), 0);
        assert_eq!(t.prefix(), PathPrefix::Crate);
    }

    #[test]
    fn truncate_to_depth_reduces_to_given_depth() {
        let r = TypeReference::new(["a", "b", "c"]);
        let t = r.truncate_to_depth(2);
        assert_eq!(t.to_path_string(), "a::b");
    }

    #[test]
    fn truncate_to_depth_equal_to_len_returns_clone() {
        let r = TypeReference::new(["a", "b", "c"]);
        let t = r.truncate_to_depth(3);
        assert_eq!(t.to_path_string(), "a::b::c");
    }

    #[test]
    fn truncate_to_depth_greater_than_len_returns_clone() {
        let r = TypeReference::new(["a", "b"]);
        let t = r.truncate_to_depth(10);
        assert_eq!(t.to_path_string(), "a::b");
    }

    #[test]
    fn truncate_to_depth_drops_suffix() {
        let r = TypeReference::new(["a", "b", "c"]).with_glob();
        let t = r.truncate_to_depth(2);
        assert!(!t.has_glob());
        assert_eq!(t.to_path_string(), "a::b");
    }

    #[test]
    fn truncate_to_depth_drops_alias() {
        let r = TypeReference::new(["a", "b", "c"]).with_alias("X");
        let t = r.truncate_to_depth(2);
        assert_eq!(t.to_path_string(), "a::b");
    }
}
