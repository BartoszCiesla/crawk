//! Parse cache for reusing parsed `syn::File` ASTs across a single analysis run.
//!
//! [`ParseCache`] avoids re-reading and re-parsing the same `.rs` file more than
//! once. `Rc` is used instead of `Arc` because `syn::File` is not `Send + Sync`
//! and the analyzer is single-threaded.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Cache mapping source file paths to their parsed `syn::File` representations.
#[derive(Clone, Default)]
pub(crate) struct ParseCache(HashMap<PathBuf, Rc<syn::File>>);

impl ParseCache {
    /// Creates an empty cache.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self(HashMap::new())
    }

    /// Returns a clone of the cached `Rc<syn::File>` for `path`, or `None` if
    /// the file has not been parsed yet.
    #[must_use]
    pub(crate) fn get(&self, path: &Path) -> Option<Rc<syn::File>> {
        self.0.get(path).map(Rc::clone)
    }

    /// Inserts a parsed file into the cache.
    pub(crate) fn insert(&mut self, path: PathBuf, file: Rc<syn::File>) {
        self.0.insert(path, file);
    }

    /// Returns the number of entries in the cache.
    #[must_use]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the cache contains no entries.
    #[must_use]
    #[allow(dead_code)]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the cached file for `path`, or calls `read_and_parse` to produce
    /// it, caches the result, and returns it.
    ///
    /// The closure receives the path and is responsible for reading the file and
    /// returning either the parsed `syn::File` or a caller-defined error.
    ///
    /// # Errors
    ///
    /// Propagates any error returned by `read_and_parse`.
    pub(crate) fn get_or_parse<E, F>(
        &mut self,
        path: &Path,
        read_and_parse: F,
    ) -> Result<Rc<syn::File>, E>
    where
        F: FnOnce(&Path) -> Result<syn::File, E>,
    {
        if let Some(cached) = self.0.get(path) {
            return Ok(Rc::clone(cached));
        }
        let file = read_and_parse(path)?;
        let rc = Rc::new(file);
        self.0.insert(path.to_path_buf(), Rc::clone(&rc));
        Ok(rc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::path::Path;

    #[test]
    fn get_or_parse_calls_closure_exactly_once_for_same_path() {
        let mut cache = ParseCache::new();
        let call_count = Cell::new(0u32);
        let path = Path::new("/fake/path.rs");

        let first = cache
            .get_or_parse(
                path,
                |_: &Path| -> Result<syn::File, std::convert::Infallible> {
                    call_count.set(call_count.get() + 1);
                    Ok(syn::parse_str("").expect("empty source parses"))
                },
            )
            .expect("first call succeeds");

        assert_eq!(call_count.get(), 1, "closure must run on first call");

        let second = cache
            .get_or_parse(
                path,
                |_: &Path| -> Result<syn::File, std::convert::Infallible> {
                    call_count.set(call_count.get() + 1);
                    Ok(syn::parse_str("").expect("empty source parses"))
                },
            )
            .expect("second call succeeds");

        assert_eq!(call_count.get(), 1, "closure must not run on second call");
        assert!(
            Rc::ptr_eq(&first, &second),
            "both calls must return same Rc allocation"
        );
    }
}
