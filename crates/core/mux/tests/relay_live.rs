//! Recovery across real HC sessions and Git worktrees, without vendor agents.
#[path = "helpers/relay_fixture.rs"]
mod fixture;
use fixture::Fixture;
use std::time::Duration;
use toolu_mux::hc::{Client, Launch, address, remote};
use toolu_mux::relay::{driver, store::Store};

#[test]
fn finished_owned_terminal_is_recovered_and_result_import_is_repeatable() {
  let world = Fixture::new().unwrap();
  let (task, repo) = world.task("finished").unwrap();
  let attempt = task.attempt.as_deref().unwrap();
  let worktree = world.worktree(&repo, attempt).unwrap();
  let name = Fixture::terminal_name(attempt);
  let owner = format!("toolu.owner={attempt}");
  let store = Store::new(&world.root);
  store
    .launched("finished", attempt, None, "launch intent")
    .unwrap();
  let client = Client::new(&world.env);
  let argv = vec!["/bin/sh".into(), "-c".into(), "printf result > result && git add result && git -c user.name=Test -c user.email=test@example.invalid commit -qm result".into()];
  let terminal = client
    .launch(&Launch {
      name: &name,
      cwd: &worktree,
      owner: &owner,
      machine: Some("loop"),
      argv: &argv,
    })
    .unwrap();
  let target = address(&terminal).unwrap();
  client
    .call(
      &["wait", target, "--timeout", "30"],
      Duration::from_secs(40),
    )
    .unwrap();
  assert_eq!(client.get(target).unwrap()["state"], "finished");
  store
    .launched("finished", attempt, Some(target), "started")
    .unwrap();
  let head = world.git(&worktree, &["rev-parse", "HEAD"]).unwrap();
  world
    .write_report(attempt, &task.request.base, &head)
    .unwrap();
  drop(store);
  driver::tick(&world.root, &world.env).unwrap();
  let reopened = Store::new(&world.root);
  let done = reopened.load("finished").unwrap();
  assert_eq!(done.phase, "done");
  assert_eq!(done.result.as_deref(), Some(head.as_str()));
  assert_eq!(
    check_import(&world, &reopened, attempt, &head).unwrap(),
    "result"
  );
}

fn check_import(
  world: &Fixture,
  store: &Store,
  attempt: &str,
  head: &str,
) -> Result<String, String> {
  let bundle = store.output("finished")?;
  let controller = world.controller.to_str().ok_or("invalid path")?;
  remote::import_recover(&world.env, controller, attempt, head, &bundle)
    .map_err(|e| format!("{e:?}"))?;
  remote::import_recover(&world.env, controller, attempt, head, &bundle)
    .map_err(|e| format!("{e:?}"))?;
  world.git(
    controller,
    &[
      "show",
      &format!("refs/toolu-transfer/{attempt}/snapshot:result"),
    ],
  )
}

#[test]
fn uncertain_launch_is_not_repeated_and_cancel_releases_only_after_stop() {
  let world = Fixture::new().unwrap();
  let (task, repo) = world.task("uncertain").unwrap();
  let attempt = task.attempt.as_deref().unwrap();
  let worktree = world.worktree(&repo, attempt).unwrap();
  let store = Store::new(&world.root);
  store
    .launched("uncertain", attempt, None, "launch intent")
    .unwrap();
  let client = Client::new(&world.env);
  let name = Fixture::terminal_name(attempt);
  assert!(client.get(&format!("loop/{name}")).is_err());
  driver::tick(&world.root, &world.env).unwrap();
  assert_eq!(store.load("uncertain").unwrap().phase, "held");
  assert!(client.get(&format!("loop/{name}")).is_err());
  let owner = format!("toolu.owner={attempt}");
  let argv = vec![
    "/bin/sh".into(),
    "-c".into(),
    "while read line; do :; done".into(),
  ];
  let terminal = client
    .launch(&Launch {
      name: &name,
      cwd: &worktree,
      owner: &owner,
      machine: Some("loop"),
      argv: &argv,
    })
    .unwrap();
  let target = address(&terminal).unwrap();
  store
    .launched("uncertain", attempt, Some(target), "recovered")
    .unwrap();
  assert_eq!(store.cancel("uncertain").unwrap().phase, "cancelling");
  assert_eq!(store.cancel("uncertain").unwrap().phase, "cancelling");
  driver::tick(&world.root, &world.env).unwrap();
  assert_eq!(client.get(target).unwrap()["state"], "finished");
  assert_eq!(store.load("uncertain").unwrap().phase, "cancelled");
  driver::tick(&world.root, &world.env).unwrap();
  assert_eq!(store.load("uncertain").unwrap().phase, "cancelled");
}

#[test]
fn absent_terminal_releases_only_if_launch_never_started() {
  let world = Fixture::new().unwrap();
  let (first, _) = world.task("before-launch").unwrap();
  let store = Store::new(&world.root);
  assert!(first.attempt.is_some());
  store.cancel("before-launch").unwrap();
  driver::tick(&world.root, &world.env).unwrap();
  assert_eq!(store.load("before-launch").unwrap().phase, "cancelled");
  let (second, _) = world.task("after-intent").unwrap();
  let attempt = second.attempt.as_deref().unwrap();
  store
    .launched("after-intent", attempt, None, "launch intent")
    .unwrap();
  store.cancel("after-intent").unwrap();
  driver::tick(&world.root, &world.env).unwrap();
  assert_eq!(store.load("after-intent").unwrap().phase, "cancelling");
}
