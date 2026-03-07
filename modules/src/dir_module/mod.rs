pub mod child_file;
pub mod child_dir;

pub fn greet() -> &'static str {
    "hello from dir_module"
}

pub fn inline_value() -> u32 {
    crate::inline_modules::nested::deep::value()
}
