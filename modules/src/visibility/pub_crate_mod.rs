pub fn greet() -> &'static str {
    "hello from pub_crate_mod"
}

pub fn parent_greet() -> &'static str {
    super::pub_mod::greet()
}
