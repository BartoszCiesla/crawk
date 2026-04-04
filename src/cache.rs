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
