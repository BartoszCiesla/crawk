pub mod pub_mod;
pub(crate) mod pub_crate_mod;
pub(super) mod pub_super_mod;
mod private_mod;
pub(crate) mod inner;

// Restricted visibility: accessible only within crate::visibility
pub(in crate::visibility) mod restricted_mod {
    use crate::nesting;

    pub fn greet() -> &'static str {
        "hello from restricted_mod"
    }

    pub fn nesting_depth() -> u32 {
        nesting::depth()
    }

    pub struct RestrictedConfig {
        pub name: &'static str,
    }

    pub const VERSION: u32 = 1;
}

pub(in crate::visibility) fn internal_helper() -> bool {
    true
}

pub(in crate::visibility) struct InternalState {
    pub value: u32,
}

pub fn demonstrate_access() -> Vec<&'static str> {
    // Exercise cross-module functions in sub-modules
    let _ = pub_crate_mod::parent_greet();
    let _ = pub_super_mod::dir_module_greet();
    let _ = private_mod::deepest_greet();
    let _ = restricted_mod::nesting_depth();

    // Exercise pub(in crate::visibility) items and inner glob import
    let _ = inner::uses_restricted();
    let _ = internal_helper();
    let state = InternalState { value: restricted_mod::VERSION };
    let _ = state.value;
    let config = restricted_mod::RestrictedConfig { name: "demo" };
    let _ = config.name;

    vec![
        pub_mod::greet(),
        pub_crate_mod::greet(),
        pub_super_mod::greet(),
        private_mod::greet(),
        restricted_mod::greet(),
    ]
}
