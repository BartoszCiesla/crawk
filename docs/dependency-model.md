# How crawk Sees Dependencies

## Overview

Every analysis command — `use`, `why`, `deps` and `check` — runs on the same
model: crawk discovers the crate's modules, parses each one with `syn`,
collects the **paths** that point at other modules of the same crate, and maps
each path to the module that owns it. `list` stops after the first step.

The model is **syntactic**. crawk reads source text; it does not compile, expand
macros, evaluate `cfg`, or infer types. A dependency exists when a module
*writes a path* to another module. That is what makes crawk fast and
dependency-free, and it is also where every blind spot below comes from.

This document describes that shared model. Per-flag behavior lives in
`crawk <COMMAND> --help`; the `check` rule language lives in
[check.md](check.md).

## Modules

### Targets

crawk analyzes the package whose `Cargo.toml` sits at the crate root (`-p`,
default: cwd), using `cargo metadata` to find its targets:

| Target kind                  | Included       | Root module name                     |
|------------------------------|----------------|--------------------------------------|
| library                      | always         | `lib`                                |
| binary                       | always         | file stem of the target's `src_path` |
| integration test             | only with `-t` | file stem of the target's `src_path` |
| example, bench, build script | never          | —                                    |

The source path comes from `cargo metadata`, so it follows the manifest, not a
fixed layout. With Cargo's default layout, `src/main.rs` becomes `main`,
`src/bin/foo.rs` becomes `foo`, and `tests/integration.rs` becomes
`integration`. A `[[bin]] name = "modules-cli"` with `path = "src/app.rs"`
becomes `app`. The root is named after the file, **not** after the target, so
a binary called `crawk` built from `src/main.rs` is `main`.

Module paths are written **without** a target prefix and without `crate::`:
`parser::visitor`, not `crate::parser::visitor` or `lib::parser::visitor`.
`list -T` shows which target each module belongs to. All targets share one
module namespace, so a binary's `mod cli` and a library's `mod cli` would be
the same node in the graph.

### Discovery

Starting from each target root, crawk follows `mod` declarations:

- `mod foo;` resolves to `foo.rs` or `foo/mod.rs` next to the declaring file.
- `mod foo { ... }` is an **inline module** — a module of its own, even though
  it shares a file with its parent. crawk tracks where inside the file it
  lives, so `self::` and `super::` resolve correctly there.
- `#[cfg(test)]` modules (including `cfg(all(test, …))`; `cfg(not(test))` is
  not a test module) are **skipped** unless `-t` is given.
- Every other `cfg` is **ignored**: `#[cfg(unix)] mod unix` and
  `#[cfg(windows)] mod windows` are both discovered, whatever the host, and so
  is code behind disabled features.
- `#[path = "..."]` is **not supported**. A module declared that way is not
  discovered (`crawk use <it>` reports "not found"), and references to it fall
  back to `lib` (see [From reference to edge](#from-reference-to-edge)).

## What counts as a reference

The parser visits each module's own items — child modules are visited
separately and never attributed to the parent — and records a path when it
appears in one of these positions:

| Position                    | Example                                        |
|-----------------------------|------------------------------------------------|
| `use` declaration           | `use crate::graph::{Cycle, DependencyGraph};`  |
| type annotation             | `fn f(x: crate::model::AnalysisResult)`        |
| trait bound                 | `T: crate::reference::Render`                  |
| implemented trait           | `impl crate::fmt::Render for X`                |
| expression path             | `crate::parser::parse(src)`, `Kind::Plain`     |
| struct literal              | `crate::model::Options { .. }`                 |
| struct / tuple pattern      | `crate::model::Kind::Named(n) => …`            |
| macro path                  | `crate::log_edge!(…)`                          |
| macro arguments             | `info!("{}", crate::version::NAME)`            |
| attribute arguments         | `#[command(version = version::VERSION)]`       |

`use` declarations count anywhere, including inside function bodies.

Macro and attribute arguments are opaque token streams, not syntax trees, so
crawk scans them for `a::b[::c…]` sequences instead. This also covers
`macro_rules!` bodies: a `$crate::foo::bar` inside a macro **definition** counts
as a dependency of the module that *defines* the macro. The modules that invoke
it do not get that dependency. Doc comments and string literals are never
scanned, so an intra-doc link like ``[`crate::foo`]`` creates no edge.

### Internal vs external

Only paths into the analyzed package are kept. A path is internal when its
first segment is:

- `crate`, `self` or `super`;
- the name of a **direct child module** of the current module
  (`use child::Item`, the 2018-edition relative form);
- the name of a **top-level module** of the crate, when the path has at least
  two segments (`sibling::Item`, the 2015-edition form, which is common under
  `use super::*;` in test modules). A lone identifier is never treated this
  way, because it is almost always a local variable;
- the **package name**, in a binary or test target that reaches into the
  library (`use crawk::Analyzer`).

Everything else — `std::`, `serde::`, other workspace crates — is dropped.

### Normalization

Before anything else, relative paths are rewritten to absolute ones:

- `self::x` in `a::b` becomes `crate::a::b::x`;
- `super::x` in `a::b` becomes `crate::a::x`, and `super::super::` climbs
  further. A `super` that would climb above the crate root stays unresolved
  (logged at debug level) and never becomes an edge;
- a bare child path `child::x` in `a::b` becomes `crate::a::b::child::x`. When a
  name is both a child and a top-level module, the child wins.

Names imported from the crate are remembered, so a token-stream path that
starts with an imported name resolves through the import:
`use crate::version;` followed by `info!("{}", version::NAME)` yields
`crate::version::NAME`.

## From reference to edge

`deps`, `why` and `check` do not work with references; they work with
**module → module edges**. Each reference is mapped to its target module like
this:

1. Take the reference's segments (`graph`, `Cycle`).
2. Find the **longest prefix that is a known module** (`graph`). That module is
   the target; the remaining segments (`Cycle`) are the **API name** shown by
   `-a` / `--show-apis`.
3. If no prefix is a known module, the target is the synthetic **`lib`** node.

Two consequences follow, and both are deliberate:

- **The edge points at the module named in the path, not at the module that
  defines the item.** `use crate::graph::Cycle` creates `rules::eval -> graph`,
  even though `Cycle` is defined in `graph::cycles` and only re-exported by
  `graph`. A facade module is exactly as visible as it is in the source: if
  callers go through the facade, the graph shows them depending on the facade.
- **Crate-root re-exports land on `lib`.** `use crawk::AnnotatedEdges` in a
  binary, or `use crate::Standalone` for an item re-exported from `lib.rs`,
  produces `… -> lib`. The same fallback catches paths into modules crawk did
  not discover (a `#[path]` module shows up as `-> lib [aliased]`).

After mapping, self-loops (`a -> a`) are dropped and duplicate edges merge,
with their API names unioned. With `--depth N`, both ends are truncated first,
so edges that collapse onto one pair, or onto a self-loop, merge or disappear
too.

## Test code

Test code is excluded by default at two levels:

- `#[cfg(test)] mod …` blocks are neither discovered nor visited — nothing
  inside them counts;
- integration-test targets (`tests/*.rs`) are not analyzed at all.

`-t` / `--include-tests` turns both on. The test module then becomes a module
of its own (`parser::tests`) with its own outgoing edges, which is why `-t`
can add nodes, edges and even new cycles to the graph.

The `cfg(test)` check applies to **modules only**. A `#[cfg(test)]` on a single
item — a helper function, a `use` — is not recognized, so its references count
in every mode.

## Blind spots

The syntactic model cannot see dependencies that are never written as a
path. It misses:

- **method calls and field access**: `x.render()` names no module; the edge
  exists only if the type or trait is imported or written out somewhere in the
  module (which is usually the case);
- **inferred types**: `let g = build();` records `build`'s module, not the
  module of the type it returns;
- **macro expansion**: derives, attribute macros and `macro_rules!` output are
  never expanded (see the definition-site rule above), and a bare
  `my_macro!()` reached through `#[macro_use]` has no path to follow;
- **conditional compilation**: every `cfg` branch counts except `cfg(test)`
  modules, so a platform- or feature-gated edge is always present;
- **`#[path]` modules**: not discovered; see [Discovery](#discovery);
- **other crates**: workspace siblings are external. Run crawk once per
  package.

In practice, a module that really uses another one almost always names it in
at least one `use` or type position, so the module graph is reliable. What is
approximate is the **API list** from `--show-apis`: it contains the names that
were written, not every item the module touches.

## How the commands use the model

| Command | Scope                                   | Groups `{A, B}`       | Globs `foo::*`                          | Output unit              |
|---------|-----------------------------------------|-----------------------|-----------------------------------------|--------------------------|
| `list`  | every target, or one subtree            | —                     | —                                       | module                   |
| `use`   | one module; `-r` adds its subtree       | kept, `-e` expands    | kept, `-G` expands                      | reference, as written    |
| `why`   | SOURCE (`-r` adds subtree) → one TARGET | always expanded       | kept (the edge goes to `foo`)           | reference                |
| `deps`  | every module in every target            | always expanded       | kept (the edge goes to `foo`)           | module → module edge     |
| `check` | same graph as `deps`                    | always expanded       | kept (the edge goes to `foo`)           | violated edge            |

Things that follow from this table:

- **`use -r` and `deps` can disagree.** `use` prints references (`crate::graph::Cycle`),
  `deps` prints edges (`rules::eval -> graph`). `use` without `-e` also shows a
  grouped import as one line. And `use` analyzes the library target unless you
  name a binary root by its file stem (`main`, not the binary's name), while `deps` always covers
  all targets.
- **`-G` exists only on `use`.** It reads the glob target's source and lists
  the items visible to the importing module: `pub`, `pub(crate)`, and
  `pub(super)` / `pub(in path)` when the importer is inside the allowed scope.
  `pub use` re-exports are listed by their exported name, and glob re-exports
  (`pub use x::*`) are not followed. The graph commands do not need `-G`,
  because a glob already names its module.
- **`why` accepts `lib` as a TARGET**, since crate-root re-exports resolve
  there. A TARGET that is not a module gives an empty answer, not an error.
- **`check` sees exactly the `deps` graph**, so every blind spot above is a
  blind spot of the architectural gate too. An edge hidden from `deps` cannot
  violate a rule.
