use std::path::Path;

use super::{REPOSITORY, apply, external, load};
use crate::markdown_cli::Finding;

fn write(root: &Path, path: &str, text: &str) {
  let full = root.join(path);
  std::fs::create_dir_all(full.parent().unwrap()).unwrap();
  std::fs::write(full, text).unwrap();
}

fn finding(file: &str, subject: &str) -> Finding {
  Finding {
    file: file.to_owned(),
    line: 3,
    subject: subject.to_owned(),
    problem: "p".to_owned(),
  }
}

#[test]
fn absent_allowlists_are_empty() {
  let dir = tempfile::tempdir().unwrap();
  let lists = load(dir.path()).unwrap();
  assert!(lists.is_empty());
  assert_eq!(
    apply(&lists, vec![finding("AGENTS.md", "toolu x")]),
    ["AGENTS.md:3: `toolu x`: p"]
  );
}

#[test]
fn an_entry_excuses_exactly_its_finding_and_a_stale_one_fails() {
  let dir = tempfile::tempdir().unwrap();
  write(
    dir.path(),
    REPOSITORY,
    r#"{"external": ["gh"], "allow": [
      {"file": "AGENTS.md", "subject": "toolu install", "reason": "the Node installer's bin"},
      {"file": "docs/a.md", "subject": "toolu gone", "reason": "was wrong once"}
    ]}"#,
  );
  write(
    dir.path(),
    "tooling/conventions/markdown-cli/jev.json",
    r#"{"external": ["sg"]}"#,
  );
  std::fs::create_dir_all(dir.path().join("plugins/jev")).unwrap();
  let lists = load(dir.path()).unwrap();
  let out = apply(
    &lists,
    vec![
      finding("AGENTS.md", "toolu install"),
      finding("AGENTS.md", "toolu installs"),
      finding("docs/b.md", "toolu gone"),
    ],
  );
  assert_eq!(
    out,
    [
      "AGENTS.md:3: `toolu installs`: p",
      "docs/b.md:3: `toolu gone`: p",
      "tooling/conventions/markdown-cli.json: stale allowance `toolu gone` in docs/a.md: it no longer fails, remove it",
    ]
  );
  let jev = external(&lists, "plugins/jev/skills/jev/SKILL.md");
  assert!(jev.contains("gh") && jev.contains("sg"));
  let other = external(&lists, "plugins/epic-orchestrator/skills/x/SKILL.md");
  assert!(other.contains("gh") && !other.contains("sg"));
}

#[test]
fn bad_allowlists_are_setup_errors_naming_the_file() {
  let cases = [
    (
      REPOSITORY,
      r#"{"allow": [{"file": "AGENTS.md", "subject": "toolu x", "reason": " "}]}"#,
      "needs a reason",
    ),
    (
      "tooling/conventions/markdown-cli/jev.json",
      r#"{"allow": [{"file": "plugins/epic-orchestrator/x.md", "subject": "toolu x", "reason": "r"}]}"#,
      "outside this allowlist's plugin",
    ),
    (REPOSITORY, r#"{"exempt": []}"#, "unknown field"),
    (
      REPOSITORY,
      r#"{"allow": [{"file": "a", "subject": "b"}]}"#,
      "missing field `reason`",
    ),
    (
      REPOSITORY,
      "not json",
      "tooling/conventions/markdown-cli.json",
    ),
  ];
  for (path, text, wanted) in cases {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plugins/jev")).unwrap();
    write(dir.path(), path, text);
    let err = load(dir.path()).unwrap_err();
    assert!(err.contains(path) && err.contains(wanted), "{err}");
  }
}

#[test]
fn an_unreadable_allowlist_is_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::create_dir_all(dir.path().join(REPOSITORY)).unwrap();
  let err = load(dir.path()).unwrap_err();
  assert!(
    err.starts_with("cannot read tooling/conventions/markdown-cli.json"),
    "{err}"
  );
}

#[test]
fn a_plugin_allowlist_must_name_a_plugin() {
  for name in ["nope.json", "jev.txt"] {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("plugins/jev")).unwrap();
    write(
      dir.path(),
      &format!("tooling/conventions/markdown-cli/{name}"),
      "{}",
    );
    let err = load(dir.path()).unwrap_err();
    assert!(
      err.contains(name) && err.contains("not `<plugin>.json`"),
      "{err}"
    );
  }
}
