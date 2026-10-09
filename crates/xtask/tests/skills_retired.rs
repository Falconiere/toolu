//! AC-9: project skills do not name the retired Bun gate commands.

use std::path::{Path, PathBuf};

const TREES: [&str; 3] = [".toolu/skills", ".claude", ".agents"];
const RETIRED: [&str; 4] = [
  "bun run test",
  "bun run guardrails",
  "bun run check:hooks-json",
  "bun run tooling/src/validate-plugin-packaging.ts",
];

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn skill_files(root: &Path) -> Result<Vec<PathBuf>, String> {
  let mut found = Vec::new();
  for tree in TREES {
    walk(&root.join(tree), &mut found)?;
  }
  Ok(found)
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) -> Result<(), String> {
  if !dir.is_dir() {
    return Ok(());
  }
  let entries = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
  for entry in entries {
    let path = entry.map_err(|err| err.to_string())?.path();
    if path.is_dir() {
      walk(&path, found)?;
    } else if path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
      found.push(path);
    }
  }
  Ok(())
}

fn retired_hits(root: &Path) -> Result<Vec<String>, String> {
  let mut hits = Vec::new();
  for path in skill_files(root)? {
    let text =
      std::fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    for command in RETIRED {
      if names_command(&text, command) {
        let rel = path.strip_prefix(root).unwrap_or(&path).display();
        hits.push(format!("{rel}: {command}"));
      }
    }
  }
  Ok(hits)
}

fn names_command(text: &str, command: &str) -> bool {
  let mut rest = text;
  while let Some(index) = rest.find(command) {
    let after = rest.get(index + command.len()..).unwrap_or("");
    if after.chars().next().is_none_or(|ch| !command_tail(ch)) {
      return true;
    }
    rest = rest.get(index + 1..).unwrap_or("");
  }
  false
}

fn command_tail(ch: char) -> bool {
  ch.is_ascii_alphanumeric() || ch == ':' || ch == '_' || ch == '-'
}

#[test]
fn skills_retired_do_not_name_retired_bun_gates() {
  assert_eq!(retired_hits(&repo()).unwrap(), Vec::<String>::new());
}

#[test]
fn skills_retired_allows_a_longer_bun_script() {
  let tmp = tempfile::tempdir().unwrap();
  let root = tmp.path();
  let skill = root.join(".toolu/skills/sample/SKILL.md");
  std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
  std::fs::write(
    &skill,
    "Run `bun run smoke:opencode-permissions` and `bun run test:unit`.\n",
  )
  .unwrap();
  assert_eq!(retired_hits(root).unwrap(), Vec::<String>::new());
  std::fs::write(&skill, "Run `bun run test`, then stop.\n").unwrap();
  assert_eq!(
    retired_hits(root).unwrap(),
    vec![".toolu/skills/sample/SKILL.md: bun run test".to_owned()]
  );
}
