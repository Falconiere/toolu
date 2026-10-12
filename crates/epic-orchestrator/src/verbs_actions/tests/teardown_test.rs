use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{finish, release};
use crate::support::matches;

#[test]
fn a_missing_record_fails_the_finish() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([("HOME", tmp.path().display().to_string())]);
  let state = tmp.path().display().to_string();
  let outcome = finish(
    &matches(&["epic", "finish", &state, "k-9", "--abandon"]),
    &env,
  );
  assert_eq!(outcome.exit, Exit::Failure);
  assert!(
    outcome
      .stderr
      .is_some_and(|text| text.contains("invalid or missing issue record"))
  );
}

#[test]
fn a_hold_is_released_once_and_unknown_or_free_keys_are_refused() {
  let tmp = tempfile::tempdir().expect("temp");
  let resources = tmp.path().join("resources");
  let env = Env::from_pairs([
    ("HOME", tmp.path().display().to_string()),
    ("TOOLU_RESOURCE_HOME", resources.display().to_string()),
  ]);
  let state = tmp.path().join("epic");
  crate::disk::write_value(
    &resources.join("registry.json"),
    &json!({"version": 1, "epics": [{"key": "e", "state_dir": state}]}),
  )
  .expect("registry");
  crate::disk::write_value(&state.join("issues/k-1.json"), &json!({"stage": "", "hold": {"kind": "coupling", "with": "k-0", "score": 0.9, "source": "jev"}})).expect("record");
  let dry = release(&matches(&["epic", "release", "k-1", "--dry-run"]), &env);
  assert_eq!(dry.exit, Exit::Success);
  let released = release(&matches(&["epic", "release", "k-1"]), &env);
  assert!(
    released
      .stdout
      .is_some_and(|text| text.contains(r#""released": true"#))
  );
  let record = crate::disk::read_value(&state.join("issues/k-1.json")).expect("record");
  assert_eq!(
    (record.get("hold"), record["coupling_released"].clone()),
    (None, json!(true))
  );
  let again = release(&matches(&["epic", "release", "k-1"]), &env);
  assert_eq!(
    again.stderr.as_deref(),
    Some("toolu epic release: k-1 is not held")
  );
  let unknown = release(&matches(&["epic", "release", "k-9"]), &env);
  assert_eq!(
    unknown.stderr.as_deref(),
    Some("toolu epic release: no registered issue k-9")
  );
}

#[test]
fn a_dry_run_over_an_incomplete_cleanup_plans_and_succeeds() {
  let tmp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([("HOME", tmp.path().display().to_string())]);
  let (checkout, worktree) = (tmp.path().join("src/r"), tmp.path().join("wt/k-1"));
  let record = json!({"key": "k-1", "repo": "o/r", "branch": "feat/1-x", "checkout": checkout,
    "worktree": worktree, "stage": "cleanup-incomplete", "kind": "claude"});
  crate::disk::write_value(&tmp.path().join("issues/k-1.json"), &record).expect("record");
  let state = tmp.path().display().to_string();
  let line = ["epic", "finish", &state, "k-1", "--abandon", "--dry-run"];
  let outcome = finish(&matches(&line), &env);
  assert_eq!(outcome.exit, Exit::Success, "{outcome:?}");
  let plan: serde_json::Value = serde_json::from_str(&outcome.stdout.expect("plan")).expect("json");
  assert_eq!(
    (plan["dry_run"].clone(), plan["stage"].clone()),
    (json!(true), json!("cleanup-incomplete"))
  );
}
