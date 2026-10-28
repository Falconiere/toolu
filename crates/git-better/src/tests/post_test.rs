//! The `commit-note` rule over real commits: it notes a fresh commit, and
//! leaves a failed, stale or disabled one alone, always allowing.

use serde_json::{Map, Value, json};
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::registry::RegistryEvent;

use super::COMMIT_NOTE;
use crate::git_support::{Repo, git};
use crate::note::NOTES_REF;

/// A repository on `feat/1-x`, read by Codex.
fn repo(config: Option<&str>) -> Repo {
  let repo = Repo::new(".codex", config);
  git(&repo.root, &["switch", "-q", "feat/1-x"]);
  repo
}

fn commit(repo: &Repo, date: Option<&str>) {
  repo.commit("README.md", "docs(readme): x", date);
}

fn after(repo: &Repo, command: &str, output: Option<Value>) -> Decision {
  let event = NormalizedEvent::ToolPost {
    session: repo.session("session-7"),
    tool: Tool {
      call_id: Text::new("c").unwrap(),
      name: Text::new("Bash").unwrap(),
      input: json!({ "command": command }).as_object().cloned().unwrap(),
    },
    output,
  };
  let raw = Map::new();
  let ctx = repo.context(Host::Codex, &raw);
  if !COMMIT_NOTE.applies(&event, &ctx) {
    return Decision::Allow;
  }
  let mut warnings = Vec::new();
  let decision = COMMIT_NOTE.run_warning(&event, &ctx, &mut warnings);
  assert_eq!(warnings, Vec::<String>::new());
  decision
}

fn note(repo: &Repo) -> Option<Value> {
  let listed = git(&repo.root, &["notes", "--ref", NOTES_REF, "list"]);
  if listed.trim().is_empty() {
    return None;
  }
  let shown = git(&repo.root, &["notes", "--ref", NOTES_REF, "show", "HEAD"]);
  serde_json::from_str(&shown).ok()
}

#[test]
fn the_rule_is_commit_note_after_bash_and_shell() {
  assert_eq!(
    (COMMIT_NOTE.spec(), COMMIT_NOTE.name(), COMMIT_NOTE.event()),
    ("git-better@toolu", "commit-note", RegistryEvent::ToolPost)
  );
}

#[test]
fn a_fresh_commit_gets_its_provenance_note() {
  let repo = repo(None);
  commit(&repo, None);
  let command = "git add -A && git commit -m 'docs(readme): x'";
  assert_eq!(
    after(&repo, command, Some(json!({"stdout": ""}))),
    Decision::Allow
  );
  let found = note(&repo).unwrap();
  assert_eq!(found["branch"], "feat/1-x");
  assert_eq!(found["base"], "main");
  assert_eq!(found["host"], "codex");
  assert_eq!(found["session"], "session-7");
  assert_eq!(found["files"][0]["path"], "README.md");
  assert_eq!(after(&repo, command, None), Decision::Allow);
}

#[test]
fn a_failed_stale_or_other_command_writes_nothing() {
  let repo = repo(None);
  commit(&repo, None);
  let failed = Some(json!({"exit_code": 1}));
  after(&repo, "git commit -m 'docs(x): y'", failed);
  after(&repo, "git status", None);
  after(&repo, "echo commit", None);
  assert_eq!(note(&repo), None);
  let stale = self::repo(None);
  commit(&stale, Some("2001-01-01T00:00:00Z"));
  after(&stale, "git commit -m 'docs(x): y'", None);
  assert_eq!(note(&stale), None);
  git(&stale.root, &["switch", "-q", "-c", "fix/2-y"]);
  after(&stale, "git commit -m 'docs(x): y'", None);
  assert_eq!(note(&stale), None);
}

#[test]
fn notes_false_turns_the_note_off() {
  let repo = repo(Some(r#"{"version":1,"gitBetter":{"notes":false}}"#));
  commit(&repo, None);
  after(&repo, "git commit -m 'docs(x): y'", None);
  assert_eq!(note(&repo), None);
}
