pub fn deepest() -> &'static str {
    "hello from level3"
}

pub fn depth() -> u32 {
    3
}

pub fn parent_depth() -> u32 {
    super::depth()
}

pub fn grandparent_depth() -> u32 {
    super::super::depth()
}

pub fn root_depth() -> u32 {
    crate::nesting::depth()
}
