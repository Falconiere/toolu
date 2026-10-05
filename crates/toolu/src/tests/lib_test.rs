use std::path::Path;

use super::{PLUGIN, ast_grep, python_quality, rust_quality, ts_quality};

#[test]
fn the_hub_is_the_toolu_plugin_and_reexports_the_rule_crates() {
  let dir = Path::new(env!("CARGO_MANIFEST_DIR")).file_name();
  assert_eq!(dir.and_then(|name| name.to_str()), Some(PLUGIN));
  let rules = [
    ts_quality::PLUGIN,
    python_quality::PLUGIN,
    rust_quality::PLUGIN,
    ast_grep::PLUGIN,
  ];
  assert_eq!(
    rules,
    ["ts-quality", "python-quality", "rust-quality", "ast-grep"]
  );
}
