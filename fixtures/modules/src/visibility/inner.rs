// A module within `crate::visibility` that glob-imports its parent scope.
// Because `visibility::inner` lies inside `crate::visibility`, all items
// declared `pub(in crate::visibility)` in the parent are accessible here.
use super::*;

pub fn uses_restricted() -> bool {
    // `internal_helper` and `InternalState` are `pub(in crate::visibility)`.
    // They resolve through the glob import above because this module is
    // within the restricted scope.
    let _ = internal_helper();
    let state = InternalState { value: restricted_mod::VERSION };
    let _ = state.value;
    true
}
