use std::time::Duration;

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::{CleanupJob, CleanupRetry, cleanup, finish_retrying, unmerged_note};
use crate::finish_rig::{Layout, layout, live, read};
use crate::herdr_state::error;
use crate::launch_rig::Rig;
use crate::paths::Paths;
use crate::reach::Reach;
use crate::support::gh;

#[test]
fn a_missing_record_is_cleanup_incomplete_with_the_token_redacted() {
  let tmp = tempfile::tempdir().expect("temp");
  let state = tmp.path().join("ghp-sentinel-dir");
  std::fs::create_dir_all(state.join("issues")).expect("issues");
  let env = Env::from_pairs([
    ("GH_TOKEN", "ghp-sentinel"),
    ("HOME", &tmp.path().display().to_string()),
  ]);
  let outcome = cleanup(&CleanupJob {
    state_dir: state,
    key: "k-1".into(),
    env,
    paths: Paths::at(tmp.path()),
    reach: Reach::live(),
  });
  assert_eq!(outcome["outcome"], "cleanup-incomplete");
  let note = outcome["note"].as_str().expect("note");
  assert!(!note.contains("ghp-sentinel"), "{note}");
  assert!(note.contains("[redacted]"), "{note}");
}

/// A merged (or open) `k-1` with a live agent that ignores `ignored` exit
/// requests, torn down with up to 3 attempts and no backoff: the result and
/// the `/exit` prompts herdr saw.
fn retried(ignored: u32, merged: bool) -> (Result<Value, String>, usize, Layout) {
  let (l, mut rig) = (layout(), Rig::new());
  rig.env = rig.env.clone().with("EPIC_AGENT_EXIT_WAIT_S", "1");
  let github = live(&l, &rig, merged);
  rig.herdr.state().ignore_exits = ignored;
  let client = gh(&github);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  let retry = CleanupRetry {
    attempts: 3,
    backoff: Duration::ZERO,
  };
  let result = finish_retrying(&l.state, "k-1", &mut ctx, retry).map(|report| report.finished);
  let exits = rig
    .herdr
    .calls()
    .iter()
    .filter(|(method, params)| method == "agent.prompt" && params["text"] == "/exit")
    .count();
  (result, exits, l)
}

#[test]
fn an_agent_that_ignores_the_first_exit_is_asked_again_and_torn_down() {
  let (result, exits, l) = retried(1, true);
  assert_eq!(
    result.map(|finished| finished["stage"].clone()),
    Ok(json!("merged"))
  );
  assert_eq!(exits, 2);
  assert!(!l.wt.exists());
}

#[test]
fn an_agent_that_never_exits_stops_after_the_last_attempt() {
  let (result, exits, l) = retried(99, true);
  assert_eq!(
    result,
    Err("after 3 attempts: still exiting: agent k-1 did not leave herdr within 1 s".to_owned())
  );
  assert_eq!(exits, 3);
  assert_eq!(read(&l.record)["stage"], "cleanup-incomplete");
}

#[test]
fn a_refusal_is_not_retried() {
  let (result, exits, _l) = retried(0, false);
  assert_eq!(result, Err("PR o/r#7 is OPEN, not MERGED".to_owned()));
  assert_eq!(exits, 0);
}

#[test]
fn the_retry_policy_reads_its_environment() {
  assert_eq!(
    CleanupRetry::from_env(&Env::from_pairs([("HOME", "/nonexistent")])),
    CleanupRetry {
      attempts: 3,
      backoff: Duration::from_millis(5000)
    }
  );
  let set = Env::from_pairs([
    ("EPIC_CLEANUP_ATTEMPTS", "0"),
    ("EPIC_CLEANUP_BACKOFF_MS", "250"),
  ]);
  assert_eq!(
    CleanupRetry::from_env(&set),
    CleanupRetry {
      attempts: 1,
      backoff: Duration::from_millis(250)
    }
  );
}

#[test]
fn a_failed_removal_names_herdr_s_error_in_the_note() {
  let (l, rig) = (layout(), Rig::new());
  let github = live(&l, &rig, true);
  rig.herdr.script(
    "worktree.remove",
    error("workspace_busy", "workspace w-1 is busy"),
  );
  let client = gh(&github);
  let mut ctx = rig.ctx();
  ctx.gh = Ok(&client);
  let retry = CleanupRetry {
    attempts: 3,
    backoff: Duration::ZERO,
  };
  let report = finish_retrying(&l.state, "k-1", &mut ctx, retry).expect("finish");
  assert_eq!(report.finished["stage"], "cleanup-incomplete");
  assert_eq!(
    unmerged_note(&report),
    "cleanup incomplete: herdr worktree.remove: workspace_busy: workspace w-1 is busy"
  );
}
