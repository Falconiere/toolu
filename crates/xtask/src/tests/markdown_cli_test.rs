use serde_json::Value;

use super::{Context, check, command_like, surfaces};

fn context() -> Context {
  let text = std::fs::read_to_string(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/cli/commands.json"
  ))
  .unwrap();
  let tree: Value = serde_json::from_str(&text).unwrap();
  Context {
    tree,
    lists: Vec::new(),
    patterns: surfaces::patterns().unwrap(),
  }
}

fn lines(file: &str, markdown: &str) -> Vec<String> {
  check(&context(), file, markdown)
    .iter()
    .map(ToString::to_string)
    .collect()
}

#[test]
fn messages_and_capitalised_words_after_toolu_are_not_commands() {
  for word in ["PostToolUse", "runtime:", "7.11.0", "<version>x"] {
    assert!(!command_like(word) || word.contains('<'), "{word}");
  }
  for word in ["epic", "--json", "<plugin>", "…", "rust-quality"] {
    assert!(command_like(word), "{word}");
  }
  assert_eq!(
    lines(
      "docs/registry.md",
      "`toolu PostToolUse dispatcher failed: <message>`\n`toolu runtime: native`\n"
    ),
    Vec::<String>::new()
  );
}

#[test]
fn fenced_and_inline_toolu_and_external_names_are_judged() {
  let mut found = lines(
    "docs/x.md",
    "Run `toolu epic strat` or:\n\n```bash\ntoolu --jsn epic planned\nmod.sh ast-grep search x\n```\n",
  );
  found.sort();
  assert_eq!(found.len(), 3, "{found:?}");
  assert!(found[0].starts_with("docs/x.md:1: `toolu epic strat`: unknown command `strat`"));
  assert!(found[1].starts_with("docs/x.md:4: `toolu --jsn epic planned`: unknown flag `--jsn`"));
  assert!(found[2].starts_with("docs/x.md:5: `mod.sh`: not `toolu`"));
}

#[test]
fn removed_surfaces_are_judged_in_plugin_markdown_only() {
  let markdown = "```bash\nbun \"$S/anything.ts\" --x\n```\nSee `hooks/dist/anything.js`.\n";
  // brainstorm owns a namespace with no placeholder verb, so its surfaces are removed.
  let found: Vec<String> = lines("plugins/brainstorm/skills/brainstorm/SKILL.md", markdown)
    .into_iter()
    .filter(|line| line.contains("removed surface"))
    .collect();
  assert_eq!(found.len(), 2, "{found:?}");
  assert!(
    found[0].contains(":2: `$S/anything.ts`: a removed surface: `toolu brainstorm` is ported")
  );
  assert!(found[1].contains(":4: `hooks/dist/anything.js`"));
  assert!(
    lines("docs/x.md", markdown)
      .iter()
      .all(|line| !line.contains("removed surface"))
  );
}

#[test]
fn a_function_defined_in_one_block_runs_in_the_next() {
  let found = lines(
    "docs/x.md",
    "```bash\njudge() {\n  :\n}\n```\n\n```bash\njudge x\nunknown y\n```\n",
  );
  assert_eq!(found.len(), 1, "{found:?}");
  assert!(found[0].starts_with("docs/x.md:9: `unknown`"), "{found:?}");
}
