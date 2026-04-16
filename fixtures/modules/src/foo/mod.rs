// Fixture for `pub(super)` visibility checks on nested file modules.
//
// `foo::bar` exposes a `pub(super)` helper visible only within `foo`'s subtree.
// `foo::other` glob-imports from `foo::bar` and must see the helper (sibling
// inside foo). `crate::baz` glob-imports the same module but must NOT see it.
pub mod bar;
pub mod other;

pub fn greet() -> &'static str {
    bar::public_fn()
}
