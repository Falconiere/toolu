//! The resource state and policy on real files (`resources.test.ts`, `pressure-policy.test.ts`).

use std::path::Path;

use toolu_runtime::env::Env;
use toolu_runtime::json::ordered::Ordered;

use super::{
  ResourcePolicy, read_resource_state, resource_home, resource_policy, safe_integer,
  update_resources,
};
use crate::ledger::jq::parse_json;

fn write(root: &Path, name: &str, body: &str) {
  std::fs::create_dir_all(root).unwrap();
  std::fs::write(root.join(name), body).unwrap();
}

#[test]
fn the_home_is_the_override_or_under_the_user_state_dir() {
  let env = Env::from_pairs([("HOME", "/home/u")]);
  assert_eq!(
    resource_home(&env),
    Path::new("/home/u/.local/state/toolu/resources")
  );
  let env = Env::from_pairs([("HOME", "/home/u"), ("TOOLU_RESOURCE_HOME", "/srv/r")]);
  assert_eq!(resource_home(&env), Path::new("/srv/r"));
}

#[test]
fn the_policy_has_defaults_and_fails_closed() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let defaults = ResourcePolicy {
    max_agents: 3,
    max_jobs: 1,
    hosts: Vec::new(),
    pressure: true,
  };
  assert_eq!(resource_policy(root), Ok(defaults));
  write(
    root,
    "policy.json",
    r#"{"maxAgents":null,"maxJobs":2,"hosts":{"claude":1},"pressure":false}"#,
  );
  let policy = resource_policy(root).unwrap();
  assert_eq!(
    (policy.max_agents, policy.max_jobs, policy.pressure),
    (3, 2, false)
  );
  assert_eq!(policy.hosts, [("claude".to_owned(), 1)]);
}

#[test]
fn an_invalid_policy_fails_closed_naming_the_problem() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  for (body, message) in [
    ("[]", "invalid resource policy"),
    (r#"{"hosts":[]}"#, "invalid resource hosts"),
    (r#"{"hosts":null}"#, "invalid resource hosts"),
    (
      r#"{"hosts":{"codex":"2"}}"#,
      "invalid resource capacity codex",
    ),
    (
      r#"{"hosts":{"codex":0},"maxAgents":"3"}"#,
      "invalid resource capacity",
    ),
    (
      r#"{"hosts":{"codex":0},"pressure":"off"}"#,
      "invalid resource pressure policy",
    ),
    (
      r#"{"hosts":{"codex":0},"maxAgents":0}"#,
      "invalid resource capacity maxAgents",
    ),
    (r#"{"maxJobs":1.5}"#, "invalid resource capacity maxJobs"),
    (
      r#"{"hosts":{"codex":0}}"#,
      "invalid resource capacity codex",
    ),
  ] {
    write(root, "policy.json", body);
    assert_eq!(resource_policy(root), Err(message.to_owned()), "{body}");
  }
  write(root, "policy.json", "{");
  assert!(resource_policy(root).is_err());
}

const LEASE: &str = r#"{"token":"t1","type":"job","key":"k","stateDir":"s","ownerPid":1,"stage":"running","createdAt":"c","heartbeatAt":"h","extra":true}"#;

#[test]
fn the_state_is_strict_and_keeps_unknown_keys_in_place() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let empty = read_resource_state(root).unwrap();
  assert_eq!(
    empty.to_text(false),
    r#"{"version":1,"leases":[],"cooldowns":{}}"#
  );
  let kept = format!(
    r#"{{"note":"x","version":1,"leases":[{LEASE}],"cooldowns":{{"claude":{{"until":5,"reason":"busy"}}}}}}"#
  );
  write(root, "state.json", &kept);
  assert_eq!(read_resource_state(root).unwrap().to_text(false), kept);
  let invalid = "invalid resource state; reconcile ownership before launching".to_owned();
  for body in [
    r#"{"version":999,"leases":[],"cooldowns":{}}"#,
    r#"{"version":1,"leases":[],"cooldowns":{"claude":{"until":"later","reason":"busy"}}}"#,
    r#"{"version":1,"leases":[{"token":"","type":"job"}],"cooldowns":{}}"#,
    r#"{"version":1,"leases":[],"cooldowns":{},"pressure":{"held":true}}"#,
    r#"{"version":1,"leases":{},"cooldowns":{}}"#,
  ] {
    write(root, "state.json", body);
    assert_eq!(read_resource_state(root), Err(invalid.clone()), "{body}");
  }
  let bad_group = LEASE.replace(r#""ownerPid":1"#, r#""ownerPid":1,"groupPid":0"#);
  write(
    root,
    "state.json",
    &format!(r#"{{"version":1,"leases":[{bad_group}],"cooldowns":{{}}}}"#),
  );
  assert_eq!(read_resource_state(root), Err(invalid));
}

#[test]
fn an_update_writes_the_state_back_even_when_it_fails() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  let failed: Result<(), String> = update_resources(root, |state| {
    state.set("note", Ordered::String("kept".to_owned()));
    Err("refused".to_owned())
  });
  assert_eq!(failed, Err("refused".to_owned()));
  let state = read_resource_state(root).unwrap();
  assert_eq!(state.get("note"), Some(&Ordered::String("kept".to_owned())));
  assert!(!root.join("state.lock").exists());
  write(root, "state.json", "{\"version\":2}");
  assert!(update_resources(root, |_| Ok(())).is_err());
}

#[test]
fn safe_integers_follow_number_is_safe_integer() {
  let value = |text: &str| parse_json(text).unwrap();
  assert_eq!(
    safe_integer(&value("9007199254740991")),
    Some(9_007_199_254_740_991)
  );
  assert_eq!(safe_integer(&value("9007199254740992")), None);
  assert_eq!(safe_integer(&value("2.0")), Some(2));
  assert_eq!(safe_integer(&value("2.5")), None);
  assert_eq!(safe_integer(&value("\"2\"")), None);
}
