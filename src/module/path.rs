//! Module for analyzing type usage in Rust source files.
//!
//! Provides a single unified type [`TypeReference`] to represent all forms of
//! type/path references in Rust code: `use` statements, fully qualified paths,
//! and relative paths (`self`, `super`, `crate`).
use std::fmt::{Display, Formatter, Result};

/// Unified type reference representing any path/type usage in Rust code.
///
/// # Examples
///
/// ```
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
    pub segments: Vec<String>,

    /// Path prefix type for relative resolution
    pub prefix: PathPrefix,

    /// Alias if renamed (`as Alias`)
    pub alias: Option<String>,

    /// True if ends with glob (`::*`)
    pub is_glob: bool,

    /// Grouped items if this is a group import (`{A, B, C}`)
    /// Each item can itself have an alias or be nested
    pub group: Option<Vec<GroupItem>>,
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
            segments: segments.into_iter().map(Into::into).collect(),
            prefix: PathPrefix::None,
            alias: None,
            is_glob: false,
            group: None,
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
        self.alias = Some(alias.into());
        self
    }

    /// Marks as glob import (`::*`).
    #[must_use]
    pub const fn with_glob(mut self) -> Self {
        self.is_glob = true;
        self
    }

    /// Sets grouped items (`{A, B, C}`).
    #[must_use]
    pub fn with_group(mut self, items: Vec<GroupItem>) -> Self {
        self.group = Some(items);
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
        self.is_glob
    }

    /// Returns true if this has a group.
    #[must_use]
    pub const fn has_group(&self) -> bool {
        self.group.is_some()
    }

    /// Returns the final name (last segment or alias if present).
    #[must_use]
    pub fn final_name(&self) -> Option<&str> {
        self.alias
            .as_deref()
            .or_else(|| self.segments.last().map(String::as_str))
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

        // Group or glob
        if let Some(ref group) = self.group {
            if !self.segments.is_empty() {
                result.push_str("::");
            }
            result.push('{');
            let items: Vec<_> = group.iter().map(ToString::to_string).collect();
            result.push_str(&items.join(", "));
            result.push('}');
        } else if self.is_glob {
            result.push_str("::*");
        }

        // Alias
        if let Some(ref alias) = self.alias {
            result.push_str(" as ");
            result.push_str(alias);
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
}
