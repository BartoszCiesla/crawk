pub mod level3;

pub fn depth() -> u32 {
    2
}

pub fn root_depth() -> u32 {
    crate::nesting::depth()
}
