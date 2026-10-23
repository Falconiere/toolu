//! Host limits that cool a host and reroute its worker, or fall back to a judgment.

fn limited_engine(root: &std::path::Path, kind: &str) -> crate::server::Engine {
  let mut engine = crate::server::Engine::open(
    crate::paths::Paths::at(root.join("engine")),
    None,
    crate::server::Fault::None,
  )
  .expect("open");
  engine.world.now_ms = 1_800_000_000_000;
  let state = root.join("epic").display().to_string();
  let mut issue = crate::model::Issue::blank("a", "one", &state, 0);
  issue.kind = kind.to_owned();
  issue.stage = "running".to_owned();
  issue.phase = "execution".to_owned();
  engine.world.issues.insert("a".into(), issue);
  engine
}

fn journal(engine: &crate::server::Engine) -> Vec<String> {
  crate::journal::tail(&engine.paths.journal_dir(), std::time::SystemTime::now())
    .expect("journal")
    .into_iter()
    .map(|row| row.name)
    .collect()
}

#[test]
fn a_rate_limited_report_cools_the_host_and_reroutes_without_a_judgment() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "claude");
  super::on_failed(
    &mut engine,
    "failed",
    "rate-limited: usage limit reached",
    "a",
  )
  .expect("cool");
  let hosts = crate::disk::read_value(&tmp.path().join("epic/hosts.json")).expect("hosts");
  assert_eq!(
    hosts["claude"]["reason"],
    "rate-limited: usage limit reached"
  );
  let pending = engine.world.issues["a"].pending.clone().expect("reroute");
  assert!(pending.action == crate::model::Action::Launch && pending.reroute);
  let names = journal(&engine);
  assert!(
    names.contains(&"host-limited".to_owned()) && names.contains(&"reroute".to_owned()),
    "{names:?}"
  );
  assert!(
    engine
      .world
      .attention
      .iter()
      .all(|item| item.kind != "failed")
  );
}

#[test]
fn a_cooldown_reaches_shared_state_for_the_configured_minutes() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "codex");
  engine.world.cooldown_min = 5;
  super::on_failed(&mut engine, "failed", "rate-limited: 429", "a").expect("cool");
  let state =
    toolu_engine::resources::store::read_resource_state(&engine.paths.root).expect("resources");
  let until = state
    .get("cooldowns")
    .and_then(|cool| cool.get("codex"))
    .and_then(|codex| codex.get("until"))
    .map(|until| until.to_text(false));
  assert_eq!(
    until,
    Some((1_800_000_000_000_u64 + 5 * 60_000).to_string())
  );
}

#[test]
fn a_limited_muse_worker_cools_muse() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "muse");
  super::on_failed(
    &mut engine,
    "failed",
    "rate-limited: model limit reached, try again later",
    "a",
  )
  .expect("cool");
  let state =
    toolu_engine::resources::store::read_resource_state(&engine.paths.root).expect("resources");
  assert!(
    state
      .get("cooldowns")
      .and_then(|cool| cool.get("muse"))
      .is_some()
  );
}

#[test]
fn an_unknown_host_is_reported_without_a_cooldown() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "gemini");
  super::on_failed(&mut engine, "failed", "rate-limited: slow down", "a").expect("cool");
  assert!(!tmp.path().join("epic/hosts.json").exists());
  assert!(journal(&engine).contains(&"host-limited".to_owned()));
  assert!(engine.world.issues["a"].pending.is_none());
  assert_eq!(engine.world.attention[0].kind, "failed");
  assert_eq!(engine.world.attention[0].note, "rate-limited: slow down");
}

#[test]
fn a_limited_worker_with_an_action_pending_is_a_failed_judgment() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "codex");
  let pending = crate::model::fresh(crate::model::Action::Checkpoint, "t".into());
  engine.world.issues.get_mut("a").expect("a").pending = Some(pending);
  super::on_failed(&mut engine, "failed", "rate-limited: 429", "a").expect("cool");
  let pending = engine.world.issues["a"].pending.as_ref().map(|p| p.action);
  assert_eq!(pending, Some(crate::model::Action::Checkpoint));
  assert_eq!(engine.world.attention[0].kind, "failed");
}

#[test]
fn a_limited_note_on_another_phase_raises_no_failed_judgment() {
  let tmp = tempfile::tempdir().expect("temp");
  let mut engine = limited_engine(tmp.path(), "gemini");
  super::on_failed(&mut engine, "blocked", "rate-limited: slow down", "a").expect("cool");
  assert!(
    engine
      .world
      .attention
      .iter()
      .all(|item| item.kind != "failed")
  );
}
