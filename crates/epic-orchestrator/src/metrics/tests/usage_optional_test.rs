use std::path::{Path, PathBuf};

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use crate::launch::record::write_record;
use crate::metrics::{tokens, usage, usage_report};

struct Case {
  host: &'static str,
  session: &'static str,
  worktree: &'static str,
  fields: &'static [&'static str],
  input: u64,
  output: u64,
}

const CASES: &[Case] = &[
  Case {
    host: "grok",
    session: "01a1bd47-c3a6-7f12-8ea9-93ee7d8e2e1f",
    worktree: "/private/tmp/grok-subagent-846",
    fields: &[
      "cachedReadTokens",
      "cacheCreationTokens",
      "reasoningTokens",
      "totalTokens",
    ],
    input: 82_655,
    output: 1_318,
  },
  Case {
    host: "muse",
    session: "01a16abb-7eef-7340-8cf2-2e0378dcf66c",
    worktree: "/private/tmp/muse-probe-603",
    fields: &["cached_tokens", "reasoning_tokens"],
    input: 683_287,
    output: 19_083,
  },
];

fn copy_tree(from: &Path, to: &Path) {
  std::fs::create_dir_all(to).unwrap();
  for entry in std::fs::read_dir(from).unwrap() {
    let entry = entry.unwrap();
    let target = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy_tree(&entry.path(), &target);
    } else {
      std::fs::copy(entry.path(), target).unwrap();
    }
  }
}

fn remove_fields(path: &Path, case: &Case) {
  let text = std::fs::read_to_string(path).unwrap();
  if case.host == "grok" {
    let mut doc: Value = serde_json::from_str(&text).unwrap();
    let session = doc["session"].as_object_mut().unwrap();
    for &field in case.fields {
      session.remove(field);
    }
    std::fs::write(path, doc.to_string()).unwrap();
  } else {
    let lines: Vec<String> = text
      .lines()
      .map(|line| {
        let mut doc: Value = serde_json::from_str(line).unwrap();
        if let Some(quantity) = doc
          .pointer_mut("/payload/event/record/quantity")
          .and_then(Value::as_object_mut)
        {
          for &field in case.fields {
            quantity.remove(field);
          }
        }
        doc.to_string()
      })
      .collect();
    std::fs::write(path, lines.join("\n") + "\n").unwrap();
  }
}

fn setup(root: &Path, case: &Case) -> (Env, PathBuf) {
  let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens");
  copy_tree(&fixture.join(case.host), &root.join(case.host));
  let paths: Vec<&str> = if case.host == "grok" {
    vec![
      "grok/sessions/%2Fprivate%2Ftmp%2Fgrok-subagent-846/01a1bd47-c3a6-7f12-8ea9-93ee7d8e2e1f/usage.json",
    ]
  } else {
    vec![
      "muse/sessions/2026/10/23/01a16abb-7eef-7340-8cf2-2e0378dcf66c/session.jsonl",
      "muse/sessions/2026/10/23/01a16abb-7eef-7340-8cf2-2e0378dcf66c/subagent/01a16abb-d62f-7e82-9494-bf99004a61fc/session.jsonl",
    ]
  };
  for path in paths {
    remove_fields(&root.join(path), case);
  }
  let grok = root.join("grok");
  let env = Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("XDG_DATA_HOME", root.to_str().unwrap()),
    ("GROK_HOME", grok.to_str().unwrap()),
  ]);
  (env, root.join("issues"))
}

#[test]
fn usage_optional_native_fields_are_not_invented_and_legacy_rows_are_unchanged() {
  for case in CASES {
    let tmp = tempfile::tempdir().unwrap();
    let (env, issues) = setup(tmp.path(), case);
    let agent = tokens::Agent {
      key: "a".into(),
      host: case.host.into(),
      session: Some(case.session.into()),
      worktree: case.worktree.into(),
      launches: 1,
    };
    let legacy = tokens::row(&agent, &env);
    for &field in case.fields {
      assert_eq!(legacy["tokens"][field], 0, "{} {field}", case.host);
    }
    let mut record = json!({"key":"a", "kind":case.host, "worktree":case.worktree, "launches":0})
      .as_object()
      .unwrap()
      .clone();
    usage::enroll(&mut record, &env).unwrap();
    usage::begin(&mut record, case.host, "2026-10-10T00:00:00Z").unwrap();
    record.insert("session_id".into(), json!(case.session));
    usage::attach(&mut record, &env).unwrap();
    usage::checkpoint(&mut record, &env, true).unwrap();
    std::fs::create_dir_all(&issues).unwrap();
    write_record(&issues.join("a.json"), &record).unwrap();
    let report = usage_report::report(tmp.path(), &env).unwrap();
    assert_eq!(
      report["totals"]["input_tokens"], case.input,
      "{}",
      case.host
    );
    assert_eq!(
      report["totals"]["output_tokens"], case.output,
      "{}",
      case.host
    );
    for &field in case.fields {
      assert!(
        report["sessions"][0]["row"]["tokens"][field].is_null(),
        "{} {field}",
        case.host
      );
    }
  }
}
