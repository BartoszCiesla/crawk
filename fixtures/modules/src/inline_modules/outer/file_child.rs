// Real location: `outer` is inline in `inline_modules.rs`, so this file-based
// `mod file_child;` must resolve here, under `outer/`, not one level up.
use crate::no_pub_items;

pub fn greet() -> &'static str {
    let _ = no_pub_items::exercise_private();
    "hello from outer's real file_child"
}
