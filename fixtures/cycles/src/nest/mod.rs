// A parent module and its own submodule referencing each other: the parent
// re-exports the child's type, the child reaches back with `use super::`. That
// is a cycle in the graph, but a structural (containment) one — `deny-cycles`
// skips it unless `deny-parent-child-cycles` is set.
mod inner;

pub use inner::InnerType;

pub struct NestType;

fn _use_inner(_i: InnerType) {}
