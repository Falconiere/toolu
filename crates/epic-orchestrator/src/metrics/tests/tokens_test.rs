use std::path::{Path, PathBuf};

use serde_json::json;
use toolu_runtime::env::Env;

use super::{agents, row};

fn fixtures() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens")
}

fn env() -> Env {
  let claude = fixtures().join("claude");
  let codex = fixtures().join("codex");
  Env::from_pairs([
    ("HOME", "/nonexistent"),
    ("CLAUDE_CONFIG_DIR", claude.to_str().unwrap()),
    ("CODEX_HOME", codex.to_str().unwrap()),
    ("XDG_DATA_HOME", fixtures().to_str().unwrap()),
  ])
}

#[test]
fn each_host_row_sums_its_own_records_or_says_why_it_cannot() {
  let agents = agents(&fixtures().join("state")).unwrap();
  let keys: Vec<&str> = agents.iter().map(|agent| agent.key.as_str()).collect();
  assert_eq!(keys, ["a", "b", "c", "d", "e"]);
  let rows: Vec<serde_json::Value> = agents.iter().map(|agent| row(agent, &env())).collect();
  assert_eq!(
    rows[0],
    json!({"key": "a", "host": "claude", "source": "claude-code transcript", "complete": true,
      "tokens": {"input_tokens": 12, "output_tokens": 23, "cache_read_input_tokens": 104,
        "cache_creation_input_tokens": 10}})
  );
  assert_eq!(
    rows[1],
    json!({"key": "b", "host": "codex", "source": "codex rollout", "complete": false,
      "tokens": {"input_tokens": 500, "cached_input_tokens": 400, "output_tokens": 50,
        "reasoning_output_tokens": 10, "total_tokens": 550}})
  );
  assert_eq!(rows[2]["status"], "unavailable");
  assert!(rows[2]["reason"].as_str().unwrap().contains("OpenCode"));
  assert_eq!(rows[3]["reason"], "no session id was captured at launch");
  assert_eq!(
    rows[4],
    json!({"key": "e", "host": "muse", "source": "muse session log", "complete": true,
      "tokens": {"input_tokens": 683_287, "output_tokens": 19_083, "cached_tokens": 558_797,
        "reasoning_tokens": 14_805}})
  );
}

#[test]
fn a_missing_transcript_is_unavailable_not_zero() {
  let mut agents = agents(&fixtures().join("state")).unwrap();
  agents[0].session = Some("33333333-3333-4333-8333-333333333333".to_owned());
  agents[1].session = Some("44444444-4444-4444-8444-444444444444".to_owned());
  assert_eq!(
    row(&agents[0], &env())["reason"],
    "no transcript for the session"
  );
  assert_eq!(
    row(&agents[1], &env())["reason"],
    "no rollout for the session"
  );
  assert!(super::agents(Path::new("/nonexistent")).is_err());
}
