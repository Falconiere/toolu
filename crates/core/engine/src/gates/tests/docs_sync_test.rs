use super::*;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

#[test]
fn attestation_must_name_the_exact_diff() {
  let dir = tempfile::tempdir().expect("tempdir");
  let file = dir.path().join("docs.json");
  std::fs::write(&file, "{\"diff_sha\":\"new\",\"decision\":\"not-needed\"}").expect("file");
  assert_eq!(attested(&file, "new").as_deref(), Some("not-needed"));
  assert_eq!(attested(&file, "old"), None);
  assert_eq!(attested(&file, ""), None);
  std::fs::write(&file, "{not json").expect("corrupt file");
  assert_eq!(attested(&file, "new"), None);
}

#[test]
fn code_needs_a_documentation_surface_not_a_release_note() {
  let config = LoadedConfig::from_data(serde_json::Map::new(), toolu_protocol::host::Host::Claude);
  let code = "crates/core/engine/src/lib.rs".to_owned();
  assert!(needs_doc(std::slice::from_ref(&code), &config));
  assert!(!needs_doc(
    &[code.clone(), "docs/registry.md".to_owned()],
    &config
  ));
  assert!(needs_doc(
    &[code, "docs/releases/v1.md".to_owned()],
    &config
  ));
  assert!(!needs_doc(&["README.md".to_owned()], &config));
}

#[test]
fn each_mode_names_the_action_the_push_will_take() {
  assert!(consequence(GateMode::Block).contains("denied"));
  assert!(consequence(GateMode::Ask).contains("Approve"));
  assert!(consequence(GateMode::Advise).contains("does not block"));
  assert_eq!(consequence(GateMode::Off), "");
}

#[test]
fn configured_doc_surfaces_replace_the_defaults() {
  let data = serde_json::json!({"docsSync": {"surfaces": ["guide/*.md"]}})
    .as_object()
    .cloned()
    .expect("object");
  let config = LoadedConfig::from_data(data, toolu_protocol::host::Host::Claude);
  let code = "src/lib.rs".to_owned();
  assert!(needs_doc(&[code.clone(), "README.md".to_owned()], &config));
  assert!(!needs_doc(&[code, "guide/setup.md".to_owned()], &config));
}

#[test]
fn a_code_only_diff_without_attestation_names_the_required_file() {
  let dir = tempfile::tempdir().expect("tempdir");
  let root = dir.path().to_path_buf();
  let state_dir = root.join("docs-state");
  let home = root.display().to_string();
  let state = state_dir.display().to_string();
  let env = Env::from_pairs([
    ("HOME", home.as_str()),
    ("DOCS_SYNC_STATE_DIR", state.as_str()),
  ]);
  let diff = Diff {
    root,
    branch: "feat/docs".to_owned(),
    sha: "exact-sha".to_owned(),
    changed: vec!["src/lib.rs".to_owned()],
  };
  let decision = emit(
    &diff,
    GateMode::Block,
    Roots::new(env, Some(Host::Claude)),
    &mut Vec::new(),
  )
  .expect("docs-sync decision");
  let Decision::Deny { reason, .. } = decision else {
    panic!("code-only diff should be denied");
  };
  assert!(
    reason
      .as_str()
      .contains(&state_dir.join("feat_docs.json").display().to_string())
  );
  assert!(reason.as_str().contains("exact-sha"));
}
