pub mod level1;

use crate::inline_modules;

pub fn depth() -> u32 {
    0
}

pub fn inline_deep_value() -> u32 {
    inline_modules::nested::deep::value()
}
