//! The `commit-note` rule over real commits: it notes a fresh commit, and
//! leaves a failed, stale or disabled one alone, always allowing.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Map, Value, json};
use tempfile::TempDir;
use toolu_protocol::decision::Decision;
use toolu_protocol::host::Host;
use toolu_protocol::normalized::{NormalizedEvent, Session, Tool};
use toolu_protocol::text::Text;
use toolu_runtime::env::Env;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::RuleContext;

use super::COMMIT_NOTE;
use crate::note::NOTES_REF;

struct Repo {
  _dir: TempDir,
  root: PathBuf,
  env: Env,
}

fn git(root: &Path, args: &[&str], date: Option<&str>) -> String {
  let mut command = Command::new("git");
  command
    .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-C"])
    .arg(root)
    .args(args);
  if let Some(date) = date {
    command.env("GIT_COMMITTER_DATE", date);
  }
  let output = command.output().unwrap();
  String::from_utf8(output.stdout).unwrap()
}

fn repo(config: Option<&str>) -> Repo {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("repo");
  std::fs::create_dir_all(root.join(".codex")).unwrap();
  git(&root, &["init", "-q", "-b", "main"], None);
  git(&root, &["switch", "-q", "-c", "feat/1-x"], None);
  if let Some(config) = config {
    std::fs::write(root.join(".codex/toolu.config.json"), config).unwrap();
  }
  let env = Env::from_pairs([
    ("PATH", std::env::var("PATH").unwrap_or_default()),
    ("HOME", dir.path().display().to_string()),
    (
      "TOOLU_CONFIG_DIR",
      dir.path().join("user").display().to_string(),
    ),
    ("TOOLU_PROJECT_DIR", root.display().to_string()),
    (
      "TOOLU_SETTINGS_DIR",
      dir.path().join("settings").display().to_string(),
    ),
  ]);
  Repo {
    _dir: dir,
    root,
    env,
  }
}

fn commit(repo: &Repo, date: Option<&str>) {
  std::fs::write(repo.root.join("README.md"), "x\n").unwrap();
  git(&repo.root, &["add", "-A"], None);
  git(&repo.root, &["commit", "-q", "-m", "docs(readme): x"], date);
}

fn text(value: &str) -> Text {
  Text::new(value).unwrap()
}

fn after(repo: &Repo, command: &str, output: Option<Value>) -> Decision {
  let event = NormalizedEvent::ToolPost {
    session: Session {
      session_id: text("session-7"),
      cwd: text(&repo.root.display().to_string()),
      project_root: text(&repo.root.display().to_string()),
      worktree: text(&repo.root.display().to_string()),
    },
    tool: Tool {
      call_id: text("c"),
      name: text("Bash"),
      input: json!({ "command": command }).as_object().cloned().unwrap(),
    },
    output,
  };
  let raw = Map::new();
  let ctx = RuleContext {
    host: Host::Codex,
    env: &repo.env,
    config_root: &repo.root,
    project_root: &repo.root,
    cwd: Some(&repo.root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  if !COMMIT_NOTE.applies(&event, &ctx) {
    return Decision::Allow;
  }
  let mut warnings = Vec::new();
  let decision = COMMIT_NOTE.run_warning(&event, &ctx, &mut warnings);
  assert_eq!(warnings, Vec::<String>::new());
  decision
}

fn note(repo: &Repo) -> Option<Value> {
  let shown = git(
    &repo.root,
    &["notes", "--ref", NOTES_REF, "show", "HEAD"],
    None,
  );
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
  git(&stale.root, &["switch", "-q", "-c", "fix/2-y"], None);
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
