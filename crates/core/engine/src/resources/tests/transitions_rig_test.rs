//! Real resource homes and leases for the migration and fence tests.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_runtime::json::ordered::Ordered;

use super::{MigrationCaps, prepare_agent_migration};
use crate::resources::leases::{LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease};
use crate::resources::store::{leases, read_resource_state, update_resources};

pub(super) const CAPS: MigrationCaps = MigrationCaps {
  host_cap: 1,
  epic_cap: Some(2),
};
pub(super) const HOSTS: &str =
  r#"{"maxAgents": 3, "maxJobs": 1, "hosts": {"claude": 1, "codex": 1}}"#;

pub(super) fn home(policy: &str) -> (tempfile::TempDir, PathBuf) {
  let dir = tempfile::tempdir().expect("temp");
  let root = dir.path().join("resources");
  std::fs::create_dir_all(&root).expect("root");
  std::fs::write(root.join("policy.json"), policy).expect("policy");
  (dir, root)
}

pub(super) fn request(
  kind: LeaseKind,
  key: &str,
  host: Option<&str>,
  worktree: Option<&Path>,
) -> LeaseRequest {
  LeaseRequest {
    kind,
    key: key.into(),
    state_dir: format!("epic-{key}"),
    host: host.map(str::to_owned),
    host_cap: None,
    epic_cap: None,
    worktree: worktree.map(|path| path.display().to_string()),
  }
}

pub(super) fn lease(root: &Path, wanted: &LeaseRequest) -> String {
  match acquire_lease(root, wanted, SystemTime::now())
    .expect("lease")
    .get("token")
  {
    Some(Ordered::String(token)) => token.clone(),
    other => panic!("no token: {other:?}"),
  }
}

pub(super) fn field(root: &Path, token: &str, key: &str) -> Option<String> {
  let state = read_resource_state(root).expect("state");
  leases(&state)
    .iter()
    .find(|lease| matches!(lease.get("token"), Some(Ordered::String(t)) if t == token))
    .and_then(|lease| match lease.get(key) {
      Some(Ordered::String(value)) => Some(value.clone()),
      _ => None,
    })
}

pub(super) fn cool(root: &Path, host: &str, offset_ms: i64) {
  let now = i64::try_from(toolu_state::time::epoch_millis(SystemTime::now())).expect("now");
  update_resources(root, |state| {
    let entry = Ordered::Object(vec![
      (
        "until".into(),
        Ordered::Number(serde_json::Number::from(now + offset_ms)),
      ),
      ("reason".into(), Ordered::String("provider limit".into())),
    ]);
    let mut cooldowns = state
      .get("cooldowns")
      .cloned()
      .unwrap_or(Ordered::Object(Vec::new()));
    cooldowns.set(host, entry);
    state.set("cooldowns", cooldowns);
    Ok(())
  })
  .expect("cool");
}

/// A running agent `moving` on claude in `worktree`.
pub(super) fn source(root: &Path, worktree: &Path) -> String {
  let token = lease(
    root,
    &request(LeaseKind::Agent, "moving", Some("claude"), Some(worktree)),
  );
  let patch = LeasePatch {
    stage: Some("running".into()),
    ..LeasePatch::default()
  };
  patch_lease(root, &token, &patch).expect("running");
  token
}

pub(super) fn prepare(root: &Path, token: &str, target: &str) -> Result<Ordered, String> {
  prepare_agent_migration(root, token, target, CAPS, SystemTime::now())
}
