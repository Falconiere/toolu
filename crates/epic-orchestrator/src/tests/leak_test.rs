use toolu_http_test_support::Reply;
use toolu_runtime::config::secrets::Secrets;

use crate::scenario_rig::Scenario;
use crate::server::Stop;

const SENTINELS: [(&str, &str); 5] = [
  ("GH_TOKEN", "ghp_sentinel0123456789"),
  ("JIRA_PAT", "jira-pat-sentinel-0123"),
  ("JIRA_API_TOKEN", "jira-api-sentinel-0123"),
  ("LINEAR_API_KEY", "lin_api_sentinel_0123"),
  ("TYPESAFE_API_KEY", "typesafe-sentinel-0123"),
];

#[test]
fn no_credential_leaks_into_the_journal_attention_or_status_after_tracker_failures() {
  let scenario = Scenario::new(&[], 1);
  let echoed: Vec<&str> = SENTINELS.iter().map(|(_, value)| *value).collect();
  let body = format!("{{\"message\":\"bad credentials {}\"}}", echoed.join(" "));
  for path in [
    "/repos/o/r/issues/1",
    "/repos/o/r/issues/1/sub_issues?per_page=100",
  ] {
    scenario
      .github
      .route(path, Reply::new(422, body.clone()))
      .expect("route");
  }
  let mut engine = scenario.engine();
  engine.env = SENTINELS
    .iter()
    .fold(engine.env.clone(), |env, (name, value)| {
      env.with(name, value)
    });
  for _ in 0..3 {
    crate::refresh::arm(&mut engine.world, "e");
    if let Some(epic) = engine.world.epics.get_mut("e") {
      epic.refresh_at_ms = 0;
    }
    assert_eq!(engine.pump().expect("pump"), Stop::Idle);
  }
  assert!(
    engine
      .world
      .attention
      .iter()
      .any(|item| item.kind == "refresh-failed")
  );
  let mut seen = String::new();
  for entry in std::fs::read_dir(scenario.paths.journal_dir()).expect("journal") {
    seen.push_str(&std::fs::read_to_string(entry.expect("entry").path()).expect("read"));
  }
  for item in &engine.world.attention {
    seen.push_str(&item.note);
  }
  let status = crate::status_api::document(&engine.world, &scenario.paths, None, &Secrets::none())
    .expect("status");
  seen.push_str(&status.to_string());
  assert!(seen.contains("refresh-error"), "{seen}");
  assert!(
    seen.contains("[redacted]"),
    "the echoed credentials were redacted: {seen}"
  );
  for (name, value) in SENTINELS {
    assert!(!seen.contains(value), "{name} leaked: {seen}");
  }
}
