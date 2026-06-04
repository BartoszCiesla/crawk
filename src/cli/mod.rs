mod overview;
mod validation;

use anyhow::Context;
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use crawk::version;
use overview::generate_after_help;
use std::fmt::{Display, Formatter, Result as FmtResult};
use std::path::PathBuf;
use tracing_subscriber::filter::LevelFilter;
use validation::{validate_depth, validate_module_path};

#[derive(Parser, Debug, Clone)]
#[command(
    version = version::VERSION,
    long_version = version::LONG_VERSION_MESSAGE,
    after_help = generate_after_help(false),
    after_long_help = generate_after_help(true)
)]
#[clap(verbatim_doc_comment)]
/// Analyze Rust module dependencies and structure
///
/// crawk analyzes your Rust codebase and reveals every module dependency — not
/// just `use` statements, but every type annotation, trait bound, struct literal,
/// and macro invocation that ties your code together.
pub(crate) struct CrawkArgs {
    #[clap(flatten)]
    options: CrawkOptions,

    #[command(subcommand)]
    pub command: CrawkCommands,
}

#[derive(Parser, Debug, Clone)]
/// Global options shared across all subcommands (path, verbosity, log file).
pub(crate) struct CrawkOptions {
    /// Specify path to the crate root directory (defaults to current directory)
    #[arg(short = 'p', long = "path")]
    path: Option<PathBuf>,

    /// Increase output verbosity (-v for info, -vv for debug)
    #[arg(short = 'v', long = "verbose", action = ArgAction::Count)]
    verbose: u8,

    /// Write logs to specified file (INFO level, or DEBUG with -vv)
    #[arg(short = 'l', long = "log-file")]
    log_file: Option<PathBuf>,
}

impl CrawkArgs {
    /// Get the crate root directory
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - No path is provided and the current directory cannot be determined
    /// - The provided path does not exist
    pub(crate) fn crate_root(&self) -> anyhow::Result<PathBuf> {
        match self.options.path.as_ref() {
            Some(path) => {
                if !path.exists() {
                    anyhow::bail!("Provided path '{}' does not exist", path.display());
                }
                Ok(path.clone())
            }
            None => std::env::current_dir().context("Failed to get current directory"),
        }
    }

    /// Get the log level filter based on verbosity
    /// # Returns
    /// LevelFilter: WARN (default), INFO (-v), or DEBUG (-vv)
    #[must_use]
    pub(crate) const fn verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 => LevelFilter::WARN,
            1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }

    /// Get the log file path if specified
    #[must_use]
    pub(crate) const fn log_file(&self) -> Option<&PathBuf> {
        self.options.log_file.as_ref()
    }

    /// Get the log level filter for file logging
    /// # Returns
    /// LevelFilter: INFO (default), or DEBUG (-vv)
    #[must_use]
    pub(crate) const fn file_verbosity(&self) -> LevelFilter {
        match self.options.verbose {
            0 | 1 => LevelFilter::INFO,
            _ => LevelFilter::DEBUG,
        }
    }
}

#[derive(Subcommand, Debug, Clone)]
pub(crate) enum CrawkCommands {
    /// Analyze a module and report all internal crate references
    ///
    /// Inspects the given module's source and reports all internal crate
    /// references — not only `use` statements, but also qualified paths in
    /// type annotations, trait bounds, struct literals, and macro invocations.
    ///
    /// MODULE_PATH: use `::` separated segments without `crate::` prefix.
    ///   e.g. "utils", "parser::visitor"
    ///
    /// Valid root targets: "lib" (library), "main" or binary name (e.g. "crawk").
    /// Any submodule path within those targets is also accepted.
    /// Note: references from binary targets use the package name as prefix
    /// (e.g. "crawk::") rather than "crate::".
    ///
    /// Empty output (exit 0) means no internal crate dependencies were found.
    ///
    /// Note: global options (-p, -v, -l) must appear before the subcommand:
    ///   crawk -p /path/to/crate use parser   ← correct
    ///   crawk use parser -p /path/to/crate   ← error
    #[clap(verbatim_doc_comment, visible_alias = "u")]
    Use(UseArgs),

    /// List all modules in the crate
    ///
    /// Discovers and displays the module structure of a Rust crate.
    /// Always lists recursively; use --depth to limit visible levels.
    ///
    /// Without MODULE_PATH: lists modules from all targets. When modules from
    /// multiple distinct targets are found, each line is prefixed with a target
    /// tag: [lib], [bin:name], [test:name]. Use --targets to always show the tag.
    ///
    /// With MODULE_PATH: scopes to that module's subtree (root included).
    /// Target tags are suppressed unless --targets is given.
    ///
    /// Empty output (exit 0) means no modules matched the filters.
    ///
    /// Note: global options (-p, -v, -l) must appear before the subcommand.
    #[clap(verbatim_doc_comment, visible_aliases = ["ls", "l"])]
    List(ListArgs),

    /// Show inter-module dependency graph for the entire crate
    ///
    /// Analyzes every module in every compilation target (lib, binaries, and
    /// optionally integration tests) and reports which modules import from
    /// which other modules. Each line is one directed dependency edge:
    ///
    ///   source -> target
    ///
    /// Both `source` and `target` are `::` separated module paths. The graph
    /// covers intra-target dependencies: references that cross target boundaries
    /// (e.g. a binary importing from the lib via the package name) are tracked
    /// as `crate::` qualified paths and therefore included automatically.
    ///
    /// The graph is built from every internal `crate::` reference found — not
    /// just `use` statements, but also qualified paths in type annotations,
    /// trait bounds, struct literals, and macro invocations.
    ///
    /// Output is sorted alphabetically by source then target. Duplicate edges
    /// (produced by depth truncation or multiple references to the same module)
    /// and self-loops are removed automatically.
    ///
    /// Empty output (exit 0) means no inter-module dependencies were found.
    ///
    /// Use --depth 1 for a bird's-eye view of top-level module relationships.
    /// Use --format dot to pipe directly into Graphviz:
    ///   crawk deps -f dot | dot -Tsvg -o deps.svg
    ///   crawk deps -d 1 -f dot | xdot -
    ///
    /// Note: global options (-p, -v, -l) must appear before the subcommand.
    #[clap(verbatim_doc_comment, visible_aliases = ["d", "dependencies"])]
    Deps(DepsArgs),

    /// Explain why a module depends on another module
    ///
    /// Shows the concrete references that create the dependency edge from
    /// SOURCE to TARGET. This is a drill-down from `deps` — it answers
    /// "what specific items does SOURCE use from TARGET?"
    ///
    /// Both SOURCE and TARGET are `::` separated module paths without
    /// `crate::` prefix. e.g. "analyzer", "parser::visitor"
    ///
    /// Empty output (exit 0) means SOURCE has no references to TARGET.
    ///
    /// Use -r to include SOURCE's submodules in the search.
    ///
    /// Note: global options (-p, -v, -l) must appear before the subcommand.
    #[clap(verbatim_doc_comment, visible_alias = "w")]
    Why(WhyArgs),
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum UseOutputFormat {
    /// Flat sorted list (default)
    #[default]
    Plain,
    /// Grouped by source module
    Grouped,
}

impl Display for UseOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Plain => f.write_str("plain"),
            Self::Grouped => f.write_str("grouped"),
        }
    }
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Parser, Debug, Clone)]
/// Arguments for the `use` subcommand — selects the module and controls analysis options.
pub(crate) struct UseArgs {
    /// Module path relative to the crate root (e.g., "utils" or "foo::bar::baz")
    #[clap(verbatim_doc_comment)]
    #[arg(value_parser = validate_module_path)]
    pub module_path: String,

    /// Recursively analyze all submodules (disabled by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'r', long = "recursive", default_value_t = false)]
    pub recursive: bool,

    /// Include modules defined in `#[cfg(test)]` blocks (excluded by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 't', long = "include-tests", default_value_t = false)]
    pub include_tests: bool,

    /// Expand grouped imports into individual paths
    ///
    /// e.g., a::{x, y} becomes a::x, a::y
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'e', long = "expand", default_value_t = false)]
    pub expand: bool,

    /// Truncate displayed dependency paths to at most N segments.
    /// Paths with ≤N segments are shown unchanged. After truncation,
    /// duplicates are removed and the result is sorted.
    ///
    /// Caveat: grouped imports (e.g. crate::foo::{A, B}) count as 1
    /// segment (just "foo"); they are not truncated even at --depth 1.
    /// Use --expand first if you want individual items truncated.
    ///
    /// e.g., --depth 1: crate::foo::Bar → crate::foo
    ///       --depth 2: crate::foo::Bar → crate::foo::Bar (unchanged)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    pub depth: Option<usize>,

    /// Output format
    ///
    /// plain   — flat sorted list (default)
    /// grouped — grouped by source module
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'f', long = "format", default_value_t = UseOutputFormat::Plain)]
    pub format: UseOutputFormat,

    /// Resolve glob imports (`use crate::foo::*`) to explicit items
    ///
    /// When enabled, glob imports are expanded into the individual items
    /// they resolve to based on the target module's public API.
    ///
    /// e.g., foo::* becomes foo::Bar, foo::Baz
    ///
    /// Note: globs inside groups (e.g. crate::foo::{Bar, *}) are only
    /// resolved when --expand is also active. Use -e -G together to fully
    /// flatten all grouped and glob imports.
    /// Unresolvable globs (private or missing targets) are kept as `*`.
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'G', long = "resolve-globs", default_value_t = false)]
    pub resolve_globs: bool,
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum ListOutputFormat {
    /// One module per line (default)
    #[default]
    Plain,
    /// ASCII table with aligned columns
    Table,
}

impl Display for ListOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Plain => f.write_str("plain"),
            Self::Table => f.write_str("table"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
/// Arguments for the `list` subcommand — lists modules in the crate.
pub(crate) struct ListArgs {
    /// Module path to scope the listing (default: entire crate)
    ///
    /// e.g., "parser" lists only parser and its submodules
    #[clap(verbatim_doc_comment)]
    #[arg(value_parser = validate_module_path)]
    pub module_path: Option<String>,

    /// Include modules defined in `#[cfg(test)]` blocks (excluded by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 't', long = "include-tests", default_value_t = false)]
    pub include_tests: bool,

    /// Show only modules at depth ≤ N (inclusive filter, not truncation).
    /// --depth 1: top-level only  (e.g. "parser")
    /// --depth 2: top-level + one nesting level (e.g. "parser", "parser::visitor")
    ///
    /// Note: unlike `use --depth`, this removes deeper modules entirely
    /// rather than truncating their paths.
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    pub depth: Option<usize>,

    /// Filter modules by case-sensitive substring match on module path.
    ///
    /// e.g., --filter parser matches "parser", "parser::visitor"
    ///       --filter Parser  → no results (module names are lowercase)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'F', long = "filter")]
    pub filter: Option<String>,

    #[clap(flatten)]
    pub display: ListDisplayArgs,

    /// Output format
    ///
    /// plain — one module per line (default)
    /// table — ASCII table with aligned columns
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'f', long = "format", default_value_t = ListOutputFormat::Plain)]
    pub format: ListOutputFormat,
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum CyclesMode {
    /// Show only modules and edges involved in cycles (default)
    #[default]
    Detect,
    #[value(
        help = "Full dependency graph with cycle edges highlighted.\n           DOT only; plain/grouped fall back to detect with a warning"
    )]
    Highlight,
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum DepsOutputFormat {
    /// Flat sorted list of edges (default)
    #[default]
    Plain,
    /// Grouped by source module with fan-out counts
    Grouped,
    /// Graphviz DOT format
    Dot,
}

impl Display for DepsOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Plain => f.write_str("plain"),
            Self::Grouped => f.write_str("grouped"),
            Self::Dot => f.write_str("dot"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
/// Arguments for the `deps` subcommand — controls depth, format, and test inclusion.
pub(crate) struct DepsArgs {
    /// Include modules defined in `#[cfg(test)]` blocks (excluded by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 't', long = "include-tests", default_value_t = false)]
    pub include_tests: bool,

    /// Truncate module paths to at most N segments.
    ///
    /// Applied to **both** source and target of every edge. Edges that become
    /// identical after truncation (including self-loops such as
    /// "parser -> parser") are silently removed.
    ///
    /// --depth 1  top-level modules only (e.g. "parser", "format")
    /// --depth 2  top-level + one nesting level (e.g. "format::flat")
    /// (omit)     full granularity — paths taken as-is from source
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'd', long = "depth", value_parser = validate_depth)]
    pub depth: Option<usize>,

    /// Output format
    ///
    /// plain   — flat sorted list of edges (default)
    /// grouped — grouped by source module with fan-out counts
    /// dot     — Graphviz DOT (pipe to `dot -Tsvg -o deps.svg` or `xdot -`)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'f', long = "format", default_value_t = DepsOutputFormat::Plain)]
    pub format: DepsOutputFormat,

    /// Detect and display dependency cycles (strongly connected components)
    ///
    /// Reports groups of modules involved in circular dependencies using
    /// Tarjan's SCC algorithm. Each reported cycle is a maximal set of
    /// modules where every module can transitively reach every other —
    /// overlapping loops are merged into one SCC, so a single cycle in
    /// the output may represent multiple intertwined circular paths.
    ///
    /// Without a value or with `detect`: shows only the modules and
    /// edges that participate in cycles.
    ///
    /// With `highlight`: renders the full dependency graph with cycle
    /// edges visually marked. Only effective with --format dot (uses
    /// orange bold edges and dashed cluster borders). Plain and grouped
    /// formats ignore highlight and fall back to detect mode with a
    /// warning.
    ///
    /// Useful combinations:
    ///   --depth 1     cycle detection at top-level module granularity;
    ///                 simplifies large SCCs into a coarser view
    ///   --show-apis   annotates cycle edges with the symbols that
    ///                 create the dependency, helping locate the cause
    ///
    /// Empty output means no cycles exist.
    #[clap(verbatim_doc_comment)]
    #[arg(
        long = "cycles",
        num_args = 0..=1,
        default_missing_value = "detect",
        value_enum,
        conflicts_with = "path",
    )]
    pub cycles: Option<CyclesMode>,

    /// Show API names (symbols) that create each dependency edge
    ///
    /// Annotates every edge with the concrete items (types, functions,
    /// traits, macros, …) that the source module references from the
    /// target module.
    ///
    /// e.g.  analyzer -> reference [GroupItem, PathPrefix, TypeReference]
    #[clap(verbatim_doc_comment)]
    #[arg(
        short = 'a',
        long = "show-apis",
        default_value_t = false,
        conflicts_with = "path"
    )]
    pub show_apis: bool,

    /// List modules with no incoming dependencies (orphans)
    ///
    /// Shows modules that no other module depends on — potential dead code
    /// or disconnected modules. Entry points (lib, main) are included
    /// since they naturally have no dependents.
    ///
    /// Respects --depth (orphans computed on truncated module paths).
    /// The --format and --show-apis flags are ignored.
    ///
    /// Empty output means every module has at least one dependent.
    #[clap(verbatim_doc_comment)]
    #[arg(long = "orphans", default_value_t = false, conflicts_with_all = ["cycles", "path"])]
    pub orphans: bool,

    /// Show all shortest dependency paths from SOURCE to TARGET
    ///
    /// Finds every path of minimum hop-count that leads from SOURCE to
    /// TARGET in the dependency graph. Multiple paths are reported when
    /// the graph contains several equally short routes (e.g. a diamond
    /// dependency where two parallel chains have the same length).
    ///
    /// Each path is printed as a chain of arrows:
    ///   source -> intermediate -> target
    ///
    /// "Shortest" is measured in edges (hops). All reported paths have
    /// the same length; longer routes are not shown.
    ///
    /// SOURCE and TARGET are `::` separated module paths, the same
    /// format used by other commands (e.g. `parser`, `format::deps_cmd`).
    ///
    /// Useful combinations:
    ///   --depth N     truncate each node to N segments; paths that
    ///                 become identical after truncation are deduplicated
    ///   --format dot  full dependency graph with path edges highlighted
    ///                 in red (bold) on top of all other edges
    ///
    /// Prints "No path from SOURCE to TARGET." to stderr and exits 0
    /// when no dependency path exists between the two modules.
    #[clap(verbatim_doc_comment)]
    #[arg(
        long = "path",
        num_args = 2,
        value_names = ["SOURCE", "TARGET"],
        value_parser = validate_module_path,
        conflicts_with_all = ["cycles", "orphans", "show_apis"],
    )]
    pub path: Option<Vec<String>>,
}

#[derive(Parser, Debug, Clone, Default)]
pub(crate) struct ListDisplayArgs {
    /// Show source file paths alongside module names
    #[clap(verbatim_doc_comment)]
    #[arg(short = 's', long = "source", default_value_t = false)]
    pub show_source: bool,

    /// Show module visibility (pub, pub(crate), pub(super), …)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'V', long = "visibility", default_value_t = false)]
    pub show_visibility: bool,

    /// Always show the target tag column ([lib], [bin:name], [test:name]).
    ///
    /// By default, the tag is shown only when modules from multiple distinct
    /// targets are present. Use this flag to force the tag in any context —
    /// including when a MODULE_PATH is given or when only one target has modules.
    ///
    /// Useful for scripting when a consistent output format is required.
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'T', long = "targets", default_value_t = false)]
    pub show_targets: bool,
}

#[derive(ValueEnum, Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum WhyOutputFormat {
    /// Flat sorted list of references (default)
    #[default]
    Plain,
    /// Grouped by source submodule
    Grouped,
}

impl Display for WhyOutputFormat {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            Self::Plain => f.write_str("plain"),
            Self::Grouped => f.write_str("grouped"),
        }
    }
}

#[derive(Parser, Debug, Clone)]
/// Arguments for the `why` subcommand — explains a dependency between two modules.
pub(crate) struct WhyArgs {
    /// Source module that depends on TARGET (e.g., "analyzer" or "parser::visitor")
    #[clap(verbatim_doc_comment)]
    #[arg(value_parser = validate_module_path)]
    pub source: String,

    /// Target module being depended on (e.g., "reference" or "discover::module_tree")
    #[clap(verbatim_doc_comment)]
    #[arg(value_parser = validate_module_path)]
    pub target: String,

    /// Recursively include SOURCE's submodules (disabled by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'r', long = "recursive", default_value_t = false)]
    pub recursive: bool,

    /// Include modules defined in `#[cfg(test)]` blocks (excluded by default)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 't', long = "include-tests", default_value_t = false)]
    pub include_tests: bool,

    /// Output format
    ///
    /// plain   — flat sorted list of references (default)
    /// grouped — grouped by source submodule (useful with -r)
    #[clap(verbatim_doc_comment)]
    #[arg(short = 'f', long = "format", default_value_t = WhyOutputFormat::Plain)]
    pub format: WhyOutputFormat,
}
