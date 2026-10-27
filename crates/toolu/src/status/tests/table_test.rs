use std::path::Path;
use std::time::Duration;

use serde_json::Value;
use toolu_engine::status::StatusSnapshot;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::process::{Spec, run};
use toolu_runtime::term::Style;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{GateFailure, record_gate_failure};

use super::render;
use crate::status::Snapshot;

fn roots(home: &Path) -> Roots {
  let home = home.display().to_string();
  Roots::new(
    Env::from_pairs([("HOME", home.as_str()), ("PATH", "/usr/bin:/bin")]),
    Some(Host::Claude),
  )
}

fn git(dir: &Path, words: &[&str]) {
  let mut argv = vec![
    "git",
    "-c",
    "user.name=t",
    "-c",
    "user.email=t@t",
    "-c",
    "commit.gpgsign=false",
  ];
  argv.extend_from_slice(words);
  let mut spec = Spec::new(argv);
  spec.cwd = Some(dir.to_path_buf());
  spec.timeout = Duration::from_secs(30);
  let ran = run(&spec).expect("git ran");
  assert_eq!(ran.exit_code, 0, "git {words:?}: {}", ran.stderr);
}

/// A real repository on `main`: one commit, one untracked file.
fn repository() -> (tempfile::TempDir, Value) {
  let dir = tempfile::tempdir().expect("tempdir");
  let repo = dir.path().join("repo");
  std::fs::create_dir_all(&repo).expect("repo dir");
  git(&repo, &["init", "-q", "-b", "main"]);
  git(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]);
  std::fs::write(repo.join("new.txt"), "new\n").expect("untracked file");
  let document = Snapshot
    .snapshot(&roots(dir.path()), &repo)
    .expect("snapshot");
  (dir, document)
}

#[test]
fn a_repository_reads_as_a_table_on_rails() {
  let (dir, document) = repository();
  let root = document["repo_root"]
    .as_str()
    .unwrap_or_default()
    .to_owned();
  assert!(root.ends_with("/repo"), "{root}");
  assert_eq!(
    render(&document, Style::Mono),
    format!(
      "┌  toolu status\n│\n\
       │  repository    {root}\n\
       │  branch        main  ↑0 ↓0\n\
       │  host          claude\n\
       │  working tree  0 staged · 0 unstaged · 1 untracked\n\
       │  gate          ○ missing\n\
       │  push review   ○ missing\n\
       │  waivers       none\n\
       └\n"
    )
  );
  assert!(!render(&document, Style::Mono).contains('\x1b'));
  drop(dir);
}

#[test]
fn outside_a_repository_the_table_says_so() {
  let dir = tempfile::tempdir().expect("tempdir");
  let document = Snapshot
    .snapshot(&roots(dir.path()), dir.path())
    .expect("snapshot");
  let table = render(&document, Style::Mono);
  assert!(
    table.contains("│  repository    not a git repository\n"),
    "{table}"
  );
  assert!(table.contains("│  branch        —\n"), "{table}");
}

/// The table of a fresh repository whose gate failed on each of `failing`
/// (recorded as the post-edit hook records it) and whose state directory holds
/// `files`.
fn table_with(failing: &[&str], files: &[(&str, &str)], style: Style) -> String {
  let (dir, document) = repository();
  let root = document["repo_root"]
    .as_str()
    .unwrap_or_default()
    .to_owned();
  let state = Path::new(&root).join(".claude/tmp");
  std::fs::create_dir_all(&state).expect("state dir");
  let mut ctx = StateCtx::new(roots(dir.path()));
  for file in failing {
    let failure = GateFailure {
      file,
      source: "ts-quality",
      reason: "lint",
      violations: "x\n",
    };
    record_gate_failure(&mut ctx, &state.join("quality-gate-status.json"), &failure);
  }
  for (name, text) in files {
    let path = state.join(name);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("state dir");
    std::fs::write(path, text).expect("state file");
  }
  let now = Snapshot
    .snapshot(&roots(dir.path()), Path::new(&root))
    .expect("snapshot");
  render(&now, style)
}

#[test]
fn a_failing_gate_a_recorded_review_and_both_waivers_show_their_state() {
  let files = [
    ("push-review/main.json", r#"{"version":2}"#),
    ("push-review/main.waiver.json", r#"{"diff_sha":"a"}"#),
    (
      "push-review/main.pending-waiver.json",
      r#"{"diff_sha":"b"}"#,
    ),
  ];
  let table = table_with(&["src/a.ts", "src/b.ts"], &files, Style::Mono);
  assert!(
    table.contains("│  gate          ✘ failing: lint (2 files)\n"),
    "{table}"
  );
  assert!(table.contains("│  push review   ✔ recorded\n"), "{table}");
  assert!(
    table.contains("│  waivers       waiver active, waiver pending\n"),
    "{table}"
  );
  assert!(table_with(&["src/a.ts"], &files, Style::Color).contains("\x1b[31m✘\x1b[0m"));
}

#[test]
fn a_malformed_gate_and_review_read_as_invalid_and_a_lone_pending_waiver_shows() {
  let table = table_with(
    &[],
    &[
      ("quality-gate-status.json", "not json"),
      ("push-review/main.json", "not json"),
      (
        "push-review/main.pending-waiver.json",
        r#"{"diff_sha":"b"}"#,
      ),
    ],
    Style::Mono,
  );
  assert!(table.contains("│  gate          ▲ invalid: "), "{table}");
  assert!(
    table.contains("│  push review   ▲ invalid: not JSON\n"),
    "{table}"
  );
  assert!(
    table.contains("│  waivers       waiver pending\n"),
    "{table}"
  );
}
