use crate::dispatch::dispatcher::Dispatcher;
use crate::dispatch::sandbox::sandbox;
use toolu_mux::relay::config;

#[test]
fn failed_submission_keeps_the_same_request_for_recovery() {
  let sb = sandbox();
  config::update(&config::root(&sb.env), |config| {
    config.relay = Some("missing-machine".into());
    Ok(())
  })
  .unwrap();
  let (plan, items) = sb.plan("claude", Some("opus"));
  assert!(
    Dispatcher::new(&sb.options(false))
      .unwrap()
      .start(&sb.start(&plan, &items))
      .is_err()
  );
  let first = sb.state()["fixer"]["groups"][0]["relayRequest"].clone();
  assert!(first["key"].as_str().is_some());
  assert!(
    first["brief"]
      .as_str()
      .unwrap()
      .contains("Make sum add its arguments")
  );
  assert!(
    Dispatcher::new(&sb.options(false))
      .unwrap()
      .wait(30)
      .is_err()
  );
  assert_eq!(sb.state()["fixer"]["groups"][0]["relayRequest"], first);
}

#[test]
fn relay_machine_only_is_persisted_and_invalid_targets_are_refused() {
  let sb = sandbox();
  config::update(&config::root(&sb.env), |config| {
    config.relay = Some("missing-machine".into());
    Ok(())
  })
  .unwrap();
  let (plan, items) = sb.plan("claude", Some("opus"));
  let mut start = sb.start(&plan, &items);
  start.machine = Some("bad/name");
  let error = Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&start)
    .unwrap_err();
  assert_eq!(error.code, "usage");
  start.machine = Some("worker-1");
  start.remote_repo = Some("relative");
  let error = Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&start)
    .unwrap_err();
  assert_eq!(error.code, "usage");
  start.remote_repo = None;
  assert!(
    Dispatcher::new(&sb.options(false))
      .unwrap()
      .start(&start)
      .is_err()
  );
  assert_eq!(
    sb.state()["fixer"]["groups"][0]["relayRequest"]["machine"],
    "worker-1"
  );
}

#[test]
fn machine_only_remains_invalid_without_relay_or_in_dry_run() {
  let sb = sandbox();
  let (plan, items) = sb.plan("claude", Some("opus"));
  let mut start = sb.start(&plan, &items);
  start.machine = Some("worker-1");
  let error = Dispatcher::new(&sb.options(false))
    .unwrap()
    .start(&start)
    .unwrap_err();
  assert_eq!(error.code, "usage");
  config::update(&config::root(&sb.env), |config| {
    config.relay = Some("relay".into());
    Ok(())
  })
  .unwrap();
  let error = Dispatcher::new(&sb.options(true))
    .unwrap()
    .start(&start)
    .unwrap_err();
  assert_eq!(error.code, "usage");
}
