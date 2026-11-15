use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::launch::record::{Record, write_record};
use crate::metrics::{usage, usage_report};

struct Case {
  host: &'static str,
  session: &'static str,
  worktree: &'static str,
  source: &'static str,
  total: u64,
  unreadable: bool,
}

const CASES: &[Case] = &[
  Case {
    host: "claude",
    session: "11111111-1111-4111-8111-111111111111",
    worktree: "/w/a",
    source: "claude/projects/-w-a/11111111-1111-4111-8111-111111111111.jsonl",
    total: 149,
    unreadable: false,
  },
  Case {
    host: "claude",
    session: "11111111-1111-4111-8111-111111111111",
    worktree: "/w/a",
    source: "claude/projects/-w-a/11111111-1111-4111-8111-111111111111/subagents/agent-1.jsonl",
    total: 149,
    unreadable: true,
  },
  Case {
    host: "codex",
    session: "22222222-2222-4222-8222-222222222222",
    worktree: "/w/b",
    source: "codex/sessions/2026/10/10/rollout-2026-10-10T00-00-00-22222222-2222-4222-8222-222222222222.jsonl",
    total: 550,
    unreadable: false,
  },
  Case {
    host: "muse",
    session: "01a16abb-7eef-7340-8cf2-2e0378dcf66c",
    worktree: "/private/tmp/muse-probe-603",
    source: "muse/sessions/2026/10/23/01a16abb-7eef-7340-8cf2-2e0378dcf66c/session.jsonl",
    total: 702_370,
    unreadable: false,
  },
  Case {
    host: "grok",
    session: "01a1bd47-c3a6-7f12-8ea9-93ee7d8e2e1f",
    worktree: "/private/tmp/grok-subagent-846",
    source: "grok/sessions/%2Fprivate%2Ftmp%2Fgrok-subagent-846/01a1bd47-c3a6-7f12-8ea9-93ee7d8e2e1f/usage.json",
    total: 83_973,
    unreadable: false,
  },
];

fn copy_tree(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let path = entry.path();
    let target = to.join(entry.file_name());
    if path.is_dir() {
      copy_tree(&path, &target);
    } else {
      std::fs::copy(path, target).unwrap();
    }
  }
}

fn setup(root: &Path, case: &Case) -> (Env, Record, PathBuf) {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens");
  copy_tree(&fixture.join(case.host), &root.join(case.host));
  let claude = root.join("claude");
  let codex = root.join("codex");
  let grok = root.join("grok");
  let env = Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CLAUDE_CONFIG_DIR", claude.to_str().unwrap()),
    ("CODEX_HOME", codex.to_str().unwrap()),
    ("XDG_DATA_HOME", root.to_str().unwrap()),
    ("GROK_HOME", grok.to_str().unwrap()),
  ]);
  let mut record = json!({"key":"issue", "kind":case.host, "worktree":case.worktree, "launches":0})
    .as_object()
    .unwrap()
    .clone();
  usage::enroll(&mut record, &env).unwrap();
  usage::begin(&mut record, case.host, "2026-10-10T00:00:00Z").unwrap();
  record.insert("session_id".into(), json!(case.session));
  (env, record, root.join(case.source))
}

fn damage(path: &Path, case: &Case, empty: bool) {
  if case.unreadable {
    std::fs::remove_file(path).unwrap();
    std::fs::create_dir(path).unwrap();
  } else {
    std::fs::write(path, if empty { "" } else { "{invalid json\n" }).unwrap();
  }
}

fn report(root: &Path, env: &Env, record: &Record) -> Value {
  let issues = root.join("issues");
  std::fs::create_dir_all(&issues).unwrap();
  write_record(&issues.join("issue.json"), record).unwrap();
  usage_report::report(root, env).unwrap()
}

#[test]
fn usage_incomplete_terminal_refresh_preserves_known_snapshot_as_stale() {
  for case in CASES {
    for empty in [true, false] {
      let tmp = tempfile::tempdir().unwrap();
      let (env, mut record, source) = setup(tmp.path(), case);
      usage::attach(&mut record, &env).unwrap();
      let observed = record["usage_history"]["attempts"][0]["observed_at"].clone();
      assert!(observed.as_str().is_some(), "{}", case.host);
      damage(&source, case, empty);
      usage::checkpoint(&mut record, &env, true).unwrap();
      let result = report(tmp.path(), &env, &record);
      assert_eq!(result["coverage"], "partial", "{}", case.source);
      assert_eq!(
        result["totals"]["total_tokens"], case.total,
        "{}",
        case.source
      );
      assert_eq!(
        result["attempts"][0]["observed_at"], observed,
        "{}",
        case.source
      );
      assert_eq!(result["attempts"][0]["final"], false, "{}", case.source);
      assert_eq!(
        result["attempts"][0]["row"]["status"], "stale",
        "{}",
        case.source
      );
      assert_ne!(result["reasons"].as_array().unwrap(), &[] as &[Value]);
    }
  }
}

#[test]
fn usage_incomplete_initial_source_is_unavailable_not_zero() {
  for case in CASES {
    for empty in [true, false] {
      let tmp = tempfile::tempdir().unwrap();
      let (env, mut record, source) = setup(tmp.path(), case);
      damage(&source, case, empty);
      usage::attach(&mut record, &env).unwrap();
      usage::checkpoint(&mut record, &env, true).unwrap();
      let result = report(tmp.path(), &env, &record);
      assert_eq!(result["coverage"], "unavailable", "{}", case.source);
      assert!(
        result["totals"]["total_tokens"].is_null(),
        "{}",
        case.source
      );
      assert_eq!(result["attempts"][0]["final"], false, "{}", case.source);
      assert_ne!(result["reasons"].as_array().unwrap(), &[] as &[Value]);
    }
  }
}
