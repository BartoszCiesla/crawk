# `crawk check` — Architectural Rule Checking

## Overview

`crawk check` enforces an **architectural contract** on a crate's internal
module dependencies. The contract is declared in a config file using two rule
kinds — named *layer groups* (`[[check.layers]]`: ordered stacks of modules
where a lower layer must not depend on a higher one) and *deny rules*
(`[[check.deny]]`: explicit bans on a specific `from -> to` edge) — and `check`
verifies every inter-module dependency edge against all of them in one pass. It
is built for CI: a clean crate exits `0`, a contract breach exits `1`, and an
operational problem exits `2`.

The config file is **required**, by design. A linter without a contract has
nothing to enforce, so a *missing* config is an operational error (exit `2`),
not a silent pass — that way a typo'd path or a forgotten file fails the build
rather than quietly reporting "clean". An *empty* `[check]` table, by contrast,
is perfectly valid: zero rules means every edge is allowed, so the crate always
passes.

A violation is **data**, not a crash: `check` reports each broken rule on its
own line and sets the exit code. Only operational problems (missing or malformed
config, a rule that names a module that does not exist, an uncovered module under
`strict-layers`) surface as errors.

## Config File Location & `--init`

`check` resolves its config in one of two ways:

- **Explicit** — with `-c <FILE>` / `--config <FILE>`, that path is used
  verbatim. If the file does not exist, `check` fails with exit `2`
  (`config file does not exist`).
- **Discovered** — without `--config`, the crate root is searched for
  `crawk.toml` first, then `.crawk.toml`. The plain name wins when both exist
  (with a warning). If neither is found, `check` fails with exit `2`; the error
  points you at the fix:

  ```
  no crawk.toml or .crawk.toml found in <crate root>; run `crawk check --init` to generate a starter config
  ```

### `crawk check --init`

`crawk check --init` scaffolds a starter `crawk.toml` in the crate root and
exits. It writes a single `[[check.layers]]` group named after the crate, listing
the crate's top-level modules alphabetically, with a comment reminding you to
reorder them (highest layer first). It deliberately does **not** guess the
hierarchy — layer ordering encodes design intent the source cannot reveal. The
next steps are spelled out on completion:

```
Scaffolded <crate root>/crawk.toml.

  Reorder the modules: highest-level layer first, lowest last.
  A lower layer must never depend on a higher one.
  Then run `crawk check`.
```

`--init` refuses to clobber an existing config: if a `crawk.toml` or
`.crawk.toml` is already present (or, with `--config`, the explicit target
exists), it errors with `config already exists; edit it directly or remove it
first` rather than overwriting your work.

## `[check]` Schema Reference

The config is a single `[check]` table. All keys are **kebab-case**, and unknown
keys are rejected (so `layerz` or a misspelling fails loudly instead of being
ignored).

| Key               | Type                  | Default | Meaning                                                                   |
|-------------------|-----------------------|---------|---------------------------------------------------------------------------|
| `layers`          | array of layer groups | `[]`    | The `[[check.layers]]` groups to enforce (see below).                     |
| `deny`            | array of deny rules   | `[]`    | The `[[check.deny]]` edge bans to enforce (see below).                    |
| `strict-layers`   | bool                  | `false` | Require every module in the crate to belong to at least one group.        |
| `deny-same-layer` | bool                  | `false` | **Default** same-layer policy for all groups; each group may override it. |

An empty `[check]` table (no keys at all) is valid and yields zero rules — a
clean pass.

### `[[check.layers]]` sub-table

Each `[[check.layers]]` entry defines one layer group:

| Key               | Type             | Meaning                                                                                          |
|-------------------|------------------|--------------------------------------------------------------------------------------------------|
| `name`            | string           | Group name. Must be **unique** across all groups. Appears in violation messages.                 |
| `order`           | array of strings | Module patterns, **highest layer first**. Each pattern covers a module and its entire subtree.   |
| `deny-same-layer` | bool (optional)  | Override the same-layer policy for this group. When omitted, inherits the `[check]`-level value. |

Notes on `order`:

- A pattern is a module path. It matches that module **and its subtree** —
  `"graph"` covers `graph::edges`, `graph::cycles`, and so on. Membership uses
  **longest-prefix match**, so a more specific pattern wins over a broader one.
- Every pattern must name a **real module** in the crate. A pattern that matches
  nothing fails the load with exit `2` (`UnknownRuleModule`), catching typos.
- A duplicate `name` across groups is an operational error, reported with its
  source line: `duplicate layer group name '<name>' (line N)`.

### `[[check.deny]]` sub-table

Each `[[check.deny]]` entry bans one dependency edge:

| Key    | Type   | Meaning                                                              |
|--------|--------|----------------------------------------------------------------------|
| `from` | string | Pattern for the **source** module of the banned edge.                |
| `to`   | string | Pattern for the **target** module of the banned edge.                |

Both keys are required; any other key is rejected. Pattern semantics differ
from `layers` — see [Deny Rules](#deny-rules--checkdeny) below.

## How Layering Works

Layers are listed **highest first**: `order[0]` is the top layer, and each later
entry sits below it. The single rule is **depend downward only** — a module may
depend on layers below it, never above.

For each dependency edge `source -> target`, `check` looks at every group that
contains **both** endpoints and compares their positions in that group's `order`:

- **target is LOWER** (later in `order`) → allowed. This is a downward
  dependency.
- **target is HIGHER** (earlier in `order`, i.e. "upward") → **violation**.
- **same layer** (same `order` index) → allowed by default; a violation only
  when `deny-same-layer = true`.

An edge whose endpoints fall in *different* groups, or in *no* group, is
**unconstrained** — there is no cross-group ordering. Layering only ever
compares two modules that share a group.

### `strict-layers`

- **What it does:** requires every module in the crate to belong to at least one
  layer group. An uncovered module is an operational error (exit `2`):
  `strict-layers: module '<module>' is not assigned to any layer`.
- **Default:** `false` — modules not named in any group are simply
  unconstrained.
- **Turn it on when** you want the architecture gate to catch *new* modules that
  slip in without being placed in the hierarchy.

### `deny-same-layer`

- **What it does:** turns a dependency between two modules in the same layer
  (same `order` index, including two modules under the same subtree pattern) into
  a violation.
- **Default:** `false` — same-layer dependencies are allowed.
- **Turn it on when** you want sibling modules within a layer to stay
  independent of each other.

`deny-same-layer` is a **per-group** policy with a crate-wide default. The key in
`[check]` sets the default for every group; a `deny-same-layer` inside a single
`[[check.layers]]` group overrides that default for that group only. This lets
one group forbid sibling coupling while another — whose siblings collaborate by
design — allows it.

```toml
[check]
deny-same-layer = false        # default for every group

# Plugins must stay independent of one another: override to true.
[[check.layers]]
name = "plugins"
order = ["plugins"]
deny-same-layer = true

# Parser internals collaborate freely; omit the key to inherit the false default.
[[check.layers]]
name = "parser-internal"
order = ["parser", "parser::visitor"]
```

How two same-layer edges resolve under this config:

- `plugins::pdf -> plugins::csv` — both sit in the single `plugins` layer.
  That group overrides `deny-same-layer = true`, so the edge is a **violation**:

  ```
  crawk check: 1 violation

    LAYER  plugins::pdf -> plugins::csv   (rule: layer 'plugins' forbids same-layer dependency (plugins::pdf -> plugins::csv))
  ```

  **The fix:** route the shared code through a lower layer both plugins depend
  on, rather than one plugin reaching into the other.
- `parser -> parser::visitor` sits in `parser-internal`, which omits the key and
  inherits the `false` default — **allowed**.

Flip the `[check]` default to `true` and the inheritance reverses: every group
denies same-layer edges unless it sets `deny-same-layer = false` for itself.

## Overlapping Groups

Groups **may overlap**: a single module can appear in several groups. Each group
is checked **independently**, so one edge can produce **one violation per group**
that forbids it, and each violation message names the offending group.

For example, given:

```toml
[[check.layers]]
name = "left"
order = ["top", "mid"]

[[check.layers]]
name = "right"
order = ["top", "mid"]
```

the edge `mid -> top` is upward in *both* `left` and `right`, so `check` reports
two violations — one attributed to `left`, one to `right`. Conversely, if an
edge is downward (or unconstrained) in a given group, that group contributes
nothing. Overlap lets you express several independent orderings over the same
modules without them interfering.

## Deny Rules — `[[check.deny]]`

A deny rule is an **explicit edge ban**: no module matching `from` may depend
on a module matching `to`. Where layering derives violations from an ordering,
`deny` names the forbidden edge directly — use it for point rules that don't
fit a stack, like "the CLI must never touch the web subsystem".

Deny rules are evaluated **independently of layers** (and of each other): every
dependency edge is tested against every deny rule, and each rule that matches
yields its own violation. In the report, `DENY` rows sort **before** `LAYER`
rows.

### Pattern semantics

Deny patterns are stricter than `layers` patterns — subtree matching is
**opt-in**, not implicit:

- A bare path matches **exactly** that module: `from = "cli"` covers `cli` but
  *not* `cli::validation`.
- An explicit `::*` suffix matches the module **and its subtree**:
  `to = "web::*"` covers `web`, `web::api`, `web::repo`, `web::service`.
- A lone `"*"` is a wildcard matching **every** module: `from = "*"` bans all
  edges into the `to` pattern, wherever they come from.

This contrast with `layers` (where `"graph"` implicitly covers `graph::edges`)
is deliberate: a ban should say exactly what it bans.

### Validation

As with layer patterns, both `from` and `to` must reference a **real module**
in the crate — a pattern that matches nothing fails the load with exit `2`,
catching typos before they silently ban nothing:

```
Error: Rule references unknown module 'clii' (in rule 'deny clii -> web')
```

### Example

```toml
# Layers govern the top-to-bottom stack; the deny rule catches a cross-group
# edge that layering deliberately leaves unconstrained.
[[check.layers]]
name = "app"
order = ["cli", "analyzer", "parser", "discover"]

[[check.deny]]
from = "cli"
to = "web::*"
```

If `cli` depends on `web::repo`, the run exits `1` and reports:

```
crawk check: 1 violation

  DENY cli -> web::repo   (rule: deny cli -> web::*)
```

The violation quotes the rule **as written**, `::*` suffix included, and names
the concrete edge that tripped it. `-a` / `--show-apis` annotates the offending
symbols, same as for layer violations:

```
  DENY cli -> web::repo [RepoType]   (rule: deny cli -> web::*)
```

## Worked Example

A complete, copy-pasteable `crawk.toml`:

```toml
[check]
# Require every module to be placed in a layer.
strict-layers = true
# Crate-wide default: same-layer dependencies are allowed unless a group opts in.
deny-same-layer = false

# Primary top-to-bottom architecture. Highest layer first:
# cli may depend on analyzer/graph/parser; parser must not depend on cli.
# This group overrides the default to forbid same-layer coupling between its
# top-level modules.
[[check.layers]]
name = "arch"
order = ["cli", "analyzer", "graph", "parser"]
deny-same-layer = true

# A second, overlapping group: within the parser subsystem, the visitor sits
# below the parser entry point. `parser` appears in both groups — each is
# checked on its own. It omits `deny-same-layer`, inheriting the false default.
[[check.layers]]
name = "parser-internal"
order = ["parser", "parser::visitor"]

# A point rule outside the stack: the CLI layer (and only it — no `::*`, so
# submodules are not covered) must never reach into the cache internals.
[[check.deny]]
from = "cli"
to = "cache::*"
```

A sample violation line (default `plain` format):

```
crawk check: 1 violation

  LAYER  parser -> cli   (rule: layer 'arch' forbids upward dependency (parser -> cli))
```

This says: in group `arch`, `parser` (a lower layer) depends on `cli` (a higher
layer), which points upward. **The fix:** invert the dependency — move the shared
type down so `cli` depends on `parser` instead of the reverse, or place the two
modules in their correct order if the hierarchy itself is wrong.

With `-a` / `--show-apis`, each line also lists the API symbols on the edge:

```
  LAYER  parser -> cli [CrawkArgs]   (rule: layer 'arch' forbids upward dependency (parser -> cli))
```

When both rule kinds fire in one run, all `DENY` rows are listed before all
`LAYER` rows.

## Exit Codes

| Code | Meaning                                                                                                                                                              |
|------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `0`  | Clean — all rules satisfied (including an empty `[check]` table).                                                                                                    |
| `1`  | One or more violations found (printed to stdout).                                                                                                                    |
| `2`  | Operational error — missing/invalid config, a rule (layer or deny) naming an unknown module, a duplicate group name, or (under `strict-layers`) an uncovered module. |

## CLI Flags

```
crawk check [OPTIONS]
```

| Flag                  | Description                                                                                    |
|-----------------------|------------------------------------------------------------------------------------------------|
| `--init`              | Scaffold a starter `crawk.toml` from discovered modules, then exit (refuses to overwrite).     |
| `-c, --config <FILE>` | Rule config path. When omitted, search the crate root for `crawk.toml`, then `.crawk.toml`.    |
| `-t, --include-tests` | Include `#[cfg(test)]` modules and test targets in the dependency graph (excluded by default). |
| `-a, --show-apis`     | Annotate each violation with the API symbols that create the offending edge.                   |
| `-f, --format <FMT>`  | Output format: `plain` (default) — one violation per line.                                     |

Global options (`-p`, `-v`, `-l`) must appear **before** the `check` subcommand.
