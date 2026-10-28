use std::path::Path;

use super::{catalog, frontmatter, heading};

fn repo_plugin(name: &str) -> std::path::PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins")
    .join(name)
}

#[test]
fn the_toolu_plugin_lists_its_skills_and_commands_namespaced() {
  let root = repo_plugin("toolu");
  let doc = catalog(&root).unwrap();
  assert_eq!(doc["plugin"], "toolu");
  let skill = doc["skills"]
    .as_array()
    .unwrap()
    .iter()
    .find(|skill| skill["id"] == "toolu:commit")
    .unwrap();
  assert_eq!(skill["name"], "toolu:commit");
  assert!(
    skill["description"]
      .as_str()
      .unwrap()
      .starts_with("Use when")
  );
  assert!(skill["content"].as_str().unwrap().starts_with("# Commit"));
  assert!(
    skill["path"]
      .as_str()
      .unwrap()
      .ends_with("skills/commit/SKILL.md")
  );
  let command = doc["commands"]
    .as_array()
    .unwrap()
    .iter()
    .find(|command| command["name"] == "toolu:commit")
    .unwrap();
  assert_eq!(command["description"], "Commit all changes");
  let template = command["template"].as_str().unwrap();
  assert!(!template.contains("${CLAUDE_PLUGIN_ROOT}"), "{template}");
  assert!(template.contains(&format!("{}/workflows/commit.md", root.display())));
  assert!(template.contains("$ARGUMENTS"));
}

#[test]
fn a_command_description_comes_from_its_frontmatter_first() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("reviewer");
  std::fs::create_dir_all(root.join("commands")).unwrap();
  std::fs::write(
    root.join("commands/review.md"),
    "---\ndescription: Review the diff.\n---\n\n# Native review\n\nRun ${CLAUDE_PLUGIN_ROOT}/x.\n",
  )
  .unwrap();
  let doc = catalog(&root).unwrap();
  let command = &doc["commands"][0];
  assert_eq!(command["name"], "reviewer:review");
  assert_eq!(command["description"], "Review the diff.");
  let template = command["template"].as_str().unwrap();
  assert!(template.starts_with("# Native review"), "{template}");
  assert!(template.contains(&format!("Run {}/x.", root.display())));
}

#[test]
fn a_plugin_without_skills_commands_or_manifest_uses_its_directory_name() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("bare");
  std::fs::create_dir_all(root.join("skills/x")).unwrap();
  std::fs::create_dir_all(root.join("commands")).unwrap();
  std::fs::write(root.join("skills/x/SKILL.md"), "no frontmatter").unwrap();
  std::fs::write(root.join("commands/notes.txt"), "ignored").unwrap();
  std::fs::write(root.join("commands/go.md"), "").unwrap();
  let doc = catalog(&root).unwrap();
  assert_eq!(doc["plugin"], "bare");
  assert_eq!(doc["skills"][0]["id"], "bare:x");
  assert_eq!(doc["skills"][0]["content"], "no frontmatter");
  assert_eq!(doc["commands"].as_array().unwrap().len(), 1);
  assert_eq!(doc["commands"][0]["description"], "go");
  let empty = dir.path().join("empty");
  std::fs::create_dir_all(&empty).unwrap();
  let none = catalog(&empty).unwrap();
  assert_eq!(none["skills"], serde_json::json!([]));
  assert_eq!(none["commands"], serde_json::json!([]));
}

#[test]
fn frontmatter_reads_plain_and_quoted_values_and_its_end() {
  let (fields, body) = frontmatter("---\nname: a\ndescription: \"b: c\"\n---\nbody\n");
  assert_eq!(fields["name"], "a");
  assert_eq!(fields["description"], "b: c");
  assert_eq!(body, "body\n");
  let (only, rest) = frontmatter("---\nname: z\n---");
  assert_eq!(only["name"], "z");
  assert_eq!(rest, "");
  let (none, text) = frontmatter("---\nname: open");
  assert!(none.is_empty());
  assert_eq!(text, "---\nname: open");
  assert_eq!(heading("intro\n# Title \n# Next"), Some("Title".to_owned()));
  assert_eq!(heading("no heading"), None);
}
