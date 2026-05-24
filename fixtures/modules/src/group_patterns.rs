// Exercises advanced group patterns inside use statements.
// Each pattern targets a specific branch of convert_use_tree.

// Pattern: self-as-alias inside a group
use crate::nesting::{self as Nest};

// Pattern: glob inside a group
use crate::file_module::{*};

// Pattern: nested path with rename inside a group
use crate::nesting::{level1::level2::level3::deepest as deep_fn};

// Pattern: nested path with glob inside a group
use crate::nesting::{level1::level2::*};

// Pattern: deeply nested path (multi-segment flattening) inside a group
use crate::nesting::{level1::level2::level3::depth as l3_depth};

pub fn demonstrate_group_patterns() -> String {
    let nest_depth = Nest::depth();
    let greeting = greet();
    let deep = deep_fn();
    let l2_depth = depth();
    let l3 = l3_depth();

    format!(
        "nest={nest_depth}, greet={greeting}, deep={deep}, l2={l2_depth}, l3={l3}"
    )
}
