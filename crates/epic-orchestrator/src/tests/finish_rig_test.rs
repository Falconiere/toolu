//! A finish test layout: a bare origin, a main clone, a linked worktree on
//! `feat/1-x`, and the issue record of `k-1`.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::{Value, json};
use toolu_engine::resources::leases::{
  LeaseKind, LeasePatch, LeaseRequest, acquire_lease, patch_lease,
};
use toolu_engine::resources::binding::bind_worktree;
use toolu_http_test_support::Fixture;
use toolu_runtime::json::ordered::Ordered;

use crate::herdr::Call;
use crate::launch_rig::Rig;
use crate::support::{git, ok};

/// The repositories and state of one finish test.
pub(crate) struct Layout {
  pub tmp: tempfile::TempDir,
  pub main: PathBuf,
  pub wt: PathBuf,
  pub state: PathBuf,
  pub record: PathBuf,
  pub resources: PathBuf,
}

/// A fresh layout with a `running` record.
/// `origin.git` with one pushed commit, its `main` clone, and `wt` on `feat/1-x`.
fn repositories(root: &Path) -> (PathBuf, PathBuf) {
  let (origin, main, wt) = (root.join("origin.git"), root.join("main"), root.join("wt"));
  crate::support::origin_with_clone(&origin, &main, &[("a.txt", "one\n")]);
  git(
    &main,
    &[
      "worktree",
      "add",
      "-q",
      "-b",
      "feat/1-x",
      &wt.display().to_string(),
      "origin/main",
    ],
  );
  (main, wt)
}

pub(crate) fn layout() -> Layout {
  let tmp = tempfile::tempdir().expect("temp");
  let root = std::fs::canonicalize(tmp.path()).expect("real");
  let (main, wt) = repositories(&root);
  let state = root.join("state");
  let record = state.join("issues").join("k-1.json");
  let layout = Layout {
    resources: root.join("resources"),
    tmp,
    main,
    wt,
    state,
    record,
  };
  let body = json!({"key": "k-1", "repo": "o/r", "branch": "feat/1-x", "checkout": layout.main, "worktree": layout.wt, "stage": "running"});
  write(&layout.record, &body);
  layout
}

/// The JSON at `path`.
pub(crate) fn read(path: &Path) -> Value {
  serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("json")
}

/// Write `body` to `path`.
pub(crate) fn write(path: &Path, body: &Value) {
  std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  std::fs::write(path, format!("{body}\n")).expect("write");
}

/// Merge `fields` into the record.
pub(crate) fn patch(layout: &Layout, fields: &Value) {
  let mut body = read(&layout.record);
  if let (Some(map), Some(extra)) = (body.as_object_mut(), fields.as_object()) {
    map.extend(extra.clone());
  }
  write(&layout.record, &body);
}

/// A codex agent lease of `k-1` on the worktree at `stage`; its token.
pub(crate) fn agent_lease(layout: &Layout, stage: &str) -> String {
  let request = LeaseRequest {
    kind: LeaseKind::Agent,
    key: "k-1".into(),
    state_dir: layout.state.display().to_string(),
    host: Some("codex".into()),
    host_cap: None,
    epic_cap: None,
    worktree: Some(layout.wt.display().to_string()),
  };
  let lease = acquire_lease(&layout.resources, &request, SystemTime::now()).expect("lease");
  let token = match lease.get("token") {
    Some(Ordered::String(token)) => token.clone(),
    _ => String::new(),
  };
  let stage = LeasePatch {
    stage: Some(stage.into()),
    ..LeasePatch::default()
  };
  patch_lease(&layout.resources, &token, &stage).expect("patch");
  token
}

/// Each lease's token and stage.
pub(crate) fn leases(layout: &Layout) -> Vec<(String, String)> {
  let state =
    toolu_engine::resources::store::read_resource_state(&layout.resources).expect("resources");
  let text = |lease: &Ordered, key: &str| match lease.get(key) {
    Some(Ordered::String(value)) => value.clone(),
    _ => String::new(),
  };
  toolu_engine::resources::store::leases(&state)
    .iter()
    .map(|lease| (text(lease, "token"), text(lease, "stage")))
    .collect()
}

/// The layout's worktree open in herdr with a live Claude agent and a merged
/// or open PR 7; the GitHub fixture.
pub(crate) fn live(l: &Layout, rig: &Rig, merged: bool) -> Fixture {
  let workspace = rig.herdr.state().open(&l.main, &l.wt, "k-1", true);
  let pane = format!("{workspace}:p1");
  let start = Call::Start {
    name: "k-1".into(),
    kind: "claude".into(),
    pane_id: pane.clone(),
    args: Vec::new(),
    timeout_ms: None,
  };
  rig.ctx().herdr.call(&start).expect("agent");
  let token = agent_lease(l, "running");
  bind_worktree(&l.resources, &l.wt, "k-1", &l.state.display().to_string()).expect("bind");
  patch(
    l,
    &json!({"workspace_id": workspace, "pane_id": pane, "agent": "k-1", "kind": "claude", "resource_root": l.resources, "lease_token": token}),
  );
  write(
    &l.state.join("status").join("k-1.json"),
    &json!({"phase": "ready", "pr": 7}),
  );
  let fixture = Fixture::start().expect("fixture");
  let state = if merged { "closed" } else { "open" };
  ok(
    &fixture,
    "/repos/o/r/pulls/7",
    &json!({"merged": merged, "state": state}),
  );
  fixture
}
