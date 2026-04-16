// Glob-import from sibling `foo::bar`. Because `helper` is `pub(super)` in
// `foo::bar`, its `super` is `foo` — and `foo::other` (sibling) is inside
// `foo`'s subtree, so `helper` must be imported here.
use super::bar::*;

pub fn use_helper() -> &'static str {
    helper()
}

pub fn use_public() -> &'static str {
    public_fn()
}
