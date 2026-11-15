use std::path::Path;

use serde_json::json;
use toolu_runtime::env::Env;

use crate::launch::{Ctx, LaunchOpts, find_issue, launch_issue};
use crate::paths::Paths;

#[test]
fn refused_first_launch_leaves_no_record_and_existing_snapshot_is_saved() {
  let tmp = tempfile::tempdir().unwrap();
  let state = tmp.path().join("state");
  let issues = state.join("issues");
  std::fs::create_dir_all(&issues).unwrap();
  std::fs::write(state.join("pool.json"), "{").unwrap();
  let mut graph = crate::support::graph_248();
  graph.state_dir = state.display().to_string();
  let issue = find_issue(&graph, "comemory-255").unwrap();
  let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/epic/tokens/codex");
  let env = Env::from_pairs([
    ("HOME", tmp.path().to_str().unwrap()),
    ("CODEX_HOME", fixtures.to_str().unwrap()),
    (
      "TOOLU_RESOURCE_HOME",
      tmp.path().join("resources").to_str().unwrap(),
    ),
  ]);
  let paths = Paths::at(tmp.path().join("engine"));
  let opts = LaunchOpts {
    kind: Some("codex".into()),
    force: true,
    ..LaunchOpts::default()
  };
  let path = issues.join("comemory-255.json");
  let launch = || {
    launch_issue(
      &graph,
      issue,
      &opts,
      &mut Ctx::live(&env, &paths, Err("no GitHub".into())),
    )
  };
  let refused = launch().unwrap_err();
  assert!(refused.message.contains("pool.json"), "{}", refused.message);
  assert!(
    !path.exists(),
    "refused first launch created {}",
    path.display()
  );

  let existing = json!({"key":"comemory-255", "kind":"codex", "worktree":"/w/b", "launches":1, "session_id":"22222222-2222-4222-8222-222222222222"});
  std::fs::write(&path, existing.to_string()).unwrap();
  let refused = launch().unwrap_err();
  assert!(refused.message.contains("pool.json"), "{}", refused.message);
  let saved = crate::launch::record::load_launch_record(&path).unwrap();
  assert_eq!(
    saved["usage_history"]["attempts"][0]["row"]["tokens"]["total_tokens"],
    550
  );
}
