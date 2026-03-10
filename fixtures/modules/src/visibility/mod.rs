pub mod pub_mod;
pub(crate) mod pub_crate_mod;
pub(super) mod pub_super_mod;
mod private_mod;

// Restricted visibility: accessible only within crate::visibility
pub(in crate::visibility) mod restricted_mod {
    pub fn greet() -> &'static str {
        "hello from restricted_mod"
    }

    pub fn nesting_depth() -> u32 {
        crate::nesting::depth()
    }
}

pub fn demonstrate_access() -> Vec<&'static str> {
    // Exercise cross-module functions in sub-modules
    let _ = pub_crate_mod::parent_greet();
    let _ = pub_super_mod::dir_module_greet();
    let _ = private_mod::deepest_greet();
    let _ = restricted_mod::nesting_depth();

    vec![
        pub_mod::greet(),
        pub_crate_mod::greet(),
        pub_super_mod::greet(),
        private_mod::greet(),
        restricted_mod::greet(),
    ]
}
