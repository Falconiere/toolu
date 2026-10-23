use std::path::{Path, PathBuf};

use serde_json::json;
use toolu_runtime::env::Env;

use super::muse;

/// The probe session of the fixture, in Muse's own data-root layout.
const ID: &str = "01a16abb-7eef-7340-8cf2-2e0378dcf66c";

fn data(root: &Path) -> Env {
  Env::from_pairs([
    ("HOME", "/nonexistent".to_owned()),
    ("XDG_DATA_HOME", root.display().to_string()),
  ])
}

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens")
}

#[test]
fn a_real_session_sums_its_reported_use_and_its_subagents_once() {
  // Main log: 35 distinct reported records (one repeated); subagent: one.
  assert_eq!(
    muse(ID, &data(&fixtures())),
    Ok((
      "muse session log",
      json!({"input_tokens": 683_287, "output_tokens": 19_083, "cached_tokens": 558_797,
        "reasoning_tokens": 14_805})
    ))
  );
}

#[test]
fn a_missing_or_unreadable_log_is_unavailable_not_zero() {
  assert_eq!(
    muse("01a16abe-bf40-7a42-a923-df25d8809d3d", &data(&fixtures())),
    Err("no session log for the session")
  );
  let tmp = tempfile::tempdir().expect("temp");
  let session = tmp.path().join("muse/sessions/2026/10/23").join(ID);
  std::fs::create_dir_all(session.join("subagent/child")).expect("dirs");
  assert_eq!(
    muse(ID, &data(tmp.path())),
    Err("the session log cannot be read")
  );
  std::fs::write(session.join("session.jsonl"), "{}\nnot json\n").expect("log");
  assert_eq!(
    muse(ID, &data(tmp.path())),
    Ok((
      "muse session log",
      json!({"input_tokens": 0, "output_tokens": 0, "cached_tokens": 0, "reasoning_tokens": 0})
    ))
  );
}
