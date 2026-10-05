use serde_json::Value;
use toolu_runtime::cli::Ctx;

use super::{SCHEMA, command, run};
use crate::tree;

fn ran(flags: &[&str], ctx: &Ctx) -> String {
  let words = std::iter::once("commands").chain(flags.iter().copied());
  let matches = command().try_get_matches_from(words).unwrap();
  run(&matches, ctx, &tree::command).stdout.unwrap()
}

#[test]
fn the_listing_has_one_line_per_visible_command() {
  let listing = ran(&[], &Ctx::default());
  let lines: Vec<&str> = listing.lines().collect();
  assert!(
    lines
      .iter()
      .any(|line| line.starts_with("toolu epic planned "))
  );
  assert!(lines.iter().any(|line| line.starts_with("toolu hook ")));
  assert!(!lines.iter().any(|line| line.starts_with("toolu jev hook")));
  let width = lines[0].find("  ").unwrap();
  assert!(lines.iter().all(|line| line.len() > width));
}

#[test]
fn json_prints_the_tree_and_schema_prints_the_schema() {
  let json = Ctx {
    json: true,
    ..Ctx::default()
  };
  let doc: Value = serde_json::from_str(&ran(&[], &json)).unwrap();
  assert_eq!(doc["schema"], "toolu.commands/v1");
  assert_eq!(ran(&["--schema"], &Ctx::default()), SCHEMA.trim_end());
  let schema: Value = serde_json::from_str(SCHEMA).unwrap();
  assert_eq!(schema["$ref"], "#/$defs/commands");
}
