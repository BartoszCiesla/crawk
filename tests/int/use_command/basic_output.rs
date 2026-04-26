use crate::common::{backtrace_filters, crawk_modules};
use insta::with_settings;
use insta_cmd::assert_cmd_snapshot;
use test_case::test_case;

// ============================================================================
// Basic output for ALL fixture modules (no flags)
// ============================================================================

#[test_case("lib"; "for lib")]
#[test_case("main"; "for main")]
#[test_case("app"; "for modules cli binary")]
#[test_case("file_module"; "for file_module")]
#[test_case("dir_module"; "for dir_module")]
#[test_case("dir_module::child_file"; "for dir_module child_file")]
#[test_case("dir_module::child_dir"; "for dir_module child_dir")]
#[test_case("inline_modules"; "for inline_modules")]
#[test_case("inline_modules::inner"; "for inline_modules inner")]
#[test_case("inline_modules::nested"; "for inline_modules nested")]
#[test_case("inline_modules::nested::deep"; "for inline_modules nested deep")]
#[test_case("custom_path"; "for custom_path")]
#[test_case("aliased"; "for aliased via path attribute")]
#[test_case("visibility"; "for visibility")]
#[test_case("visibility::pub_mod"; "for visibility pub_mod")]
#[test_case("visibility::pub_crate_mod"; "for visibility pub_crate_mod")]
#[test_case("visibility::pub_super_mod"; "for visibility pub_super_mod")]
#[test_case("visibility::private_mod"; "for visibility private_mod")]
#[test_case("nesting"; "for nesting")]
#[test_case("nesting::level1"; "for nesting level1")]
#[test_case("nesting::level1::level2"; "for nesting level1 level2")]
#[test_case("nesting::level1::level2::level3"; "for nesting level1 level2 level3")]
#[test_case("reexports"; "for reexports")]
#[test_case("mixed"; "for mixed")]
#[test_case("mixed::file_child"; "for mixed file_child")]
#[test_case("item_visibility"; "for item_visibility")]
#[test_case("glob_patterns"; "for glob_patterns")]
#[test_case("advanced_globs"; "for advanced_globs")]
#[test_case("glob_showcase"; "for glob_showcase")]
#[test_case("empty_module"; "for empty_module")]
#[test_case("no_pub_items"; "for no_pub_items")]
#[test_case("uses_empty_glob"; "for uses_empty_glob")]
#[test_case("foo"; "for foo")]
#[test_case("foo::bar"; "for foo bar")]
#[test_case("foo::other"; "for foo other")]
#[test_case("baz"; "for baz")]
#[test_case("nonexisting"; "for nonexisting module")]
fn should_modules_use_provide_output(module: &str) {
    let snapshot_name = format!("modules_{}", module.replace("::", "__"));

    with_settings!({
        filters => backtrace_filters(),
    }, {
        assert_cmd_snapshot!(snapshot_name, crawk_modules().arg("use").arg(module));
    });
}
