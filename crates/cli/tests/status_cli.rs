//! `toolu status --json` matches `Snapshot` on a real repository.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};
use toolu_engine::status::StatusSnapshot;
use toolu_hub::status::Snapshot;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{GateFailure, record_gate_failure};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

const CLEAR: &str = "TOOLU_CONFIG_DIR TOOLU_USER_CONFIG_DIR CLAUDE_CONFIG_DIR CLAUDE_PROJECT_DIR \
CODEX_HOME CURSOR_PROJECT_DIR HERMES_HOME XDG_CONFIG_HOME TOOLU_OPENCODE_HOME TOOLU_PROJECT_DIR \
GIT_DIR GIT_WORK_TREE";

fn kept(path: &Path) -> bool {
  std::fs::create_dir_all(path).is_ok()
}

fn git(dir: &Path, args: &[&str]) -> bool {
  Command::new("git")
    .args(args)
    .current_dir(dir)
    .status()
    .is_ok_and(|status| status.success())
}

fn toolu(dir: &Path, home: &Path, project: Option<&Path>) -> Option<std::process::Output> {
  let mut command = Command::new(TOOLU);
  command
    .args(["--host", "claude", "--json", "status"])
    .current_dir(dir)
    .env("HOME", home)
    .env("PATH", "/usr/bin:/bin");
  if let Some(project) = project {
    command.env("TOOLU_PROJECT_DIR", project);
  }
  for key in CLEAR.split(' ') {
    if project.is_some() && key == "TOOLU_PROJECT_DIR" {
      continue;
    }
    command.env_remove(key);
  }
  command.output().ok()
}

fn roots(home: &Path, project: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap_or("")),
      ("PATH", "/usr/bin:/bin"),
      ("TOOLU_PROJECT_DIR", project.to_str().unwrap_or("")),
    ]),
    Some(Host::Claude),
  )
}

fn record(ctx: &mut StateCtx, gate: &Path, file: &str, violations: &str) {
  record_gate_failure(
    ctx,
    gate,
    &GateFailure {
      file,
      source: "ts-quality",
      reason: "lint",
      violations,
    },
  );
}

fn committed_repo() -> Option<(PathBuf, PathBuf)> {
  let root = tempfile::tempdir().ok()?.into_path();
  let repo = root.join("repo");
  let home = root.join("home");
  if !(kept(&repo) && kept(&home)) {
    return None;
  }
  let ready = [
    &["init"][..],
    &["checkout", "-B", "feat/x"][..],
    &["config", "user.email", "t@example.com"][..],
    &["config", "user.name", "test"][..],
  ]
  .into_iter()
  .all(|args| git(&repo, args))
    && std::fs::write(repo.join("tracked.txt"), "a\n").is_ok()
    && std::fs::write(repo.join(".gitignore"), ".claude/\n").is_ok()
    && git(&repo, &["add", "tracked.txt", ".gitignore"])
    && git(&repo, &["commit", "-m", "init"])
    && std::fs::write(repo.join("tracked.txt"), "a\nb\n").is_ok()
    && std::fs::write(repo.join("new.txt"), "c\n").is_ok();
  ready.then_some((repo, home))
}

fn write_state(repo: &Path, home: &Path) -> Option<Roots> {
  let bound = roots(home, repo);
  let mut ctx = StateCtx::new(bound.clone());
  let gate = repo.join(".claude/tmp/quality-gate-status.json");
  if !kept(gate.parent()?) {
    return None;
  }
  record(&mut ctx, &gate, "src/a.ts", "a\n");
  record(&mut ctx, &gate, "src/b.ts", "b\n");
  let review = repo.join(".claude/tmp/push-review");
  let body = r#"{"version":2,"diff_sha":"abc","review_round":1,"reviewers":["code-review"],"findings_count":0,"reviewed_files":["src/a.ts"]}"#;
  if !kept(&review)
    || std::fs::write(review.join("feat_x.json"), body).is_err()
    || std::fs::write(review.join("feat_x.waiver.json"), r#"{"diff_sha":"abc"}"#).is_err()
  {
    return None;
  }
  Some(bound)
}

fn at<'a>(value: &'a Value, path: &[&str]) -> Option<&'a Value> {
  path.iter().try_fold(value, |cursor, key| match cursor {
    Value::Array(items) => key.parse::<usize>().ok().and_then(|index| items.get(index)),
    Value::Object(_) => cursor.get(*key),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => None,
  })
}

fn assert_recorded(cli: &Value) {
  assert_eq!(at(cli, &["branch"]), Some(&json!("feat/x")));
  assert_eq!(at(cli, &["working_tree", "staged"]), Some(&json!(0)));
  assert_eq!(at(cli, &["working_tree", "unstaged"]), Some(&json!(1)));
  assert_eq!(at(cli, &["working_tree", "untracked"]), Some(&json!(1)));
  assert_eq!(at(cli, &["gate", "status"]), Some(&json!("failing")));
  assert_eq!(
    at(cli, &["gate", "entries"])
      .and_then(Value::as_array)
      .map(Vec::len),
    Some(2)
  );
  assert_eq!(
    at(cli, &["gate", "entries", "0", "violations"]),
    Some(&json!("a\n"))
  );
  assert_eq!(at(cli, &["push_review", "state"]), Some(&json!("recorded")));
  assert_eq!(
    at(cli, &["push_review", "document", "review_round"]),
    Some(&json!(1))
  );
  assert_eq!(
    at(cli, &["push_review", "document", "findings_count"]),
    Some(&json!(0))
  );
  assert_eq!(
    at(cli, &["push_review", "document", "reviewed_files"]),
    Some(&json!(["src/a.ts"]))
  );
  let path = at(cli, &["push_review", "path"])
    .and_then(Value::as_str)
    .unwrap_or("");
  assert!(path.contains("feat_x.json"), "{path}");
  assert_eq!(
    at(cli, &["waivers", "waiver", "diff_sha"]),
    Some(&json!("abc"))
  );
  assert!(schema_accepts(cli), "{cli}");
}

#[test]
fn status_matches_the_snapshot_on_a_real_repository() {
  let Some((repo, home)) = committed_repo() else {
    panic!("repo");
  };
  // macOS reports `/private/var` for a `/var` cwd. Use one path for both sides.
  let repo = std::fs::canonicalize(&repo).unwrap_or(repo);
  let Some(bound) = write_state(&repo, &home) else {
    panic!("state");
  };
  let Some(output) = toolu(&repo, &home, Some(&repo)) else {
    panic!("spawn");
  };
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let cli: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
  let snap = Snapshot.snapshot(&bound, &repo).expect("snapshot");
  assert_eq!(cli, snap);
  assert_recorded(&cli);
}

#[test]
fn a_non_repo_project_still_reads_the_gate() {
  let root = tempfile::tempdir().expect("temp").into_path();
  let project = root.join("project");
  let home = root.join("home");
  assert!(kept(&project) && kept(&home));
  let gate = project.join(".claude/tmp/quality-gate-status.json");
  assert!(kept(gate.parent().unwrap_or(project.as_path())));
  assert!(
    std::fs::write(
      &gate,
      r#"{"status":"passing","source":"s","updatedAt":"u"}"#,
    )
    .is_ok()
  );
  let Some(output) = toolu(&project, &home, Some(&project)) else {
    panic!("spawn");
  };
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let cli: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
  assert_eq!(cli["repo_root"], "");
  assert_eq!(cli["branch"], "");
  assert_eq!(cli["gate"]["status"], "passing");
  assert_eq!(cli["push_review"]["state"], "missing");
  assert!(cli["waivers"]["waiver"].is_null());
}

#[test]
fn no_project_root_leaves_gate_and_review_missing() {
  let root = tempfile::tempdir().expect("temp").into_path();
  let dir = root.join("empty");
  let home = root.join("home");
  assert!(kept(&dir) && kept(&home));
  let Some(output) = toolu(&dir, &home, None) else {
    panic!("spawn");
  };
  assert!(
    output.status.success(),
    "{}",
    String::from_utf8_lossy(&output.stderr)
  );
  let cli: Value = serde_json::from_str(&String::from_utf8_lossy(&output.stdout)).expect("json");
  assert_eq!(cli["repo_root"], "");
  assert_eq!(cli["ahead"], 0);
  assert_eq!(cli["gate"]["status"], "missing");
  assert_eq!(cli["push_review"]["state"], "missing");
  assert!(cli["waivers"]["pending"].is_null());
}

fn schema_accepts(document: &Value) -> bool {
  let schema_file = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands.schema.json");
  let Ok(text) = std::fs::read_to_string(schema_file) else {
    return false;
  };
  let Ok(schema) = serde_json::from_str::<Value>(&text) else {
    return false;
  };
  let (Some(schema_id), Some(defs)) = (schema.get("$schema"), schema.get("$defs")) else {
    return false;
  };
  let status_schema = json!({
    "$schema": schema_id,
    "$defs": defs,
    "$ref": "#/$defs/status",
  });
  jsonschema::validator_for(&status_schema).is_ok_and(|validator| validator.is_valid(document))
}
