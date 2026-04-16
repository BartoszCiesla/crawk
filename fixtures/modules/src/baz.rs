// Glob-import from `crate::foo::bar` from OUTSIDE `foo`'s subtree. Only
// `public_fn` is visible; `pub(super) fn helper` is hidden because `super`
// of `foo::bar` is `foo`, and `baz` does not live under `foo`.
use crate::foo::bar::*;

pub fn use_public() -> &'static str {
    public_fn()
}
