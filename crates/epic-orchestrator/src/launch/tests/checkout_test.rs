use serde_json::json;
use toolu_http_test_support::Fixture;
use toolu_runtime::env::Env;

use super::{guard_budget, prepare_checkout, shell_join};
use crate::launch::failure::argv;
use crate::support::{gh, graph_248, ok};

#[test]
fn shell_lines_quote_only_what_the_shell_would_split() {
  assert_eq!(
    shell_join(&argv(&["git", "-C", "/a b", "it's", "", "x=1,2@3%4+5"])),
    "git -C '/a b' 'it'\\''s' '' x=1,2@3%4+5"
  );
}

#[test]
fn a_dry_checkout_logs_clone_and_fetch_and_a_bad_repo_is_refused() {
  let env = Env::from_pairs([("HOME", "/tmp"), ("PATH", "")]);
  let mut graph = graph_248();
  graph.clone_root = "/clones".into();
  let mut log = Vec::new();
  let issue = graph.find("homebrew-tap-1").expect("issue").clone();
  let (checkout, base) = prepare_checkout(
    &graph,
    &issue,
    true,
    &mut log,
    (&env, Err("offline".into())),
  )
  .expect("dry");
  assert_eq!(
    (checkout.as_str(), base.as_str()),
    ("/clones/homebrew-tap", "main")
  );
  assert_eq!(
    log,
    [
      "gh repo clone Falconiere/homebrew-tap /clones/homebrew-tap",
      "# dry-run: skip the default-branch lookup; assume main",
      "git -C /clones/homebrew-tap fetch origin main",
    ]
  );
  let mut bad = issue.clone();
  bad.repo = "nope".into();
  let refused = prepare_checkout(
    &graph,
    &bad,
    true,
    &mut Vec::new(),
    (&env, Err("offline".into())),
  )
  .expect_err("bad");
  assert_eq!(refused.message, "bad repo: nope");
  let live = prepare_checkout(
    &graph,
    &graph.find("comemory-255").expect("issue").clone(),
    false,
    &mut Vec::new(),
    (&env, Err("no token".into())),
  );
  assert_eq!(
    live.map_err(|err| err.message),
    Err("cannot read the default branch of Falconiere/comemory: no token".to_owned())
  );
}

#[test]
fn the_budget_guard_refuses_under_a_floor_and_passes_otherwise() {
  let fixture = Fixture::start().expect("fixture");
  let client = gh(&fixture);
  let env = Env::from_pairs([("HOME", "/tmp")]);
  let budget = |core: u64, graphql: u64| json!({"resources": {"core": {"remaining": core, "limit": 5000, "reset": 1_900_000_000}, "graphql": {"remaining": graphql, "limit": 5000, "reset": 1_800_000_000}}});
  ok(&fixture, "/rate_limit", &budget(4000, 4000));
  assert_eq!(guard_budget(Ok(&client), &env), Ok(()));
  ok(&fixture, "/rate_limit", &budget(999, 4000));
  let low = guard_budget(Ok(&client), &env).expect_err("low");
  assert_eq!(
    low.message,
    "GitHub core budget 999/5000 below floor 1000; resets by 2027-01-15T08:00:00.000Z. Launch later, or pass --force."
  );
  ok(&fixture, "/rate_limit", &budget(4000, 10));
  let floor = env.clone().with("EPIC_GH_GRAPHQL_FLOOR", "5");
  assert_eq!(guard_budget(Ok(&client), &floor), Ok(()));
  assert!(
    guard_budget(Ok(&client), &env)
      .expect_err("graphql")
      .message
      .starts_with("GitHub GraphQL budget 10/5000 below floor 500")
  );
  assert_eq!(guard_budget(Err("offline".into()), &env), Ok(()));
}

#[cfg(unix)]
#[test]
fn concurrent_launches_of_one_missing_repo_clone_it_once() {
  use std::os::unix::fs::PermissionsExt;
  let tmp = tempfile::tempdir().expect("temp");
  let bin = tmp.path().join("bin");
  std::fs::create_dir_all(&bin).expect("bin");
  let calls = tmp.path().join("calls.log");
  let stub = bin.join("gh");
  let script = format!(
    "#!/bin/sh\necho \"$@\" >> {}\nsleep 1\nmkdir -p \"$4/.git\"\n",
    calls.display()
  );
  std::fs::write(&stub, script).expect("stub");
  std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).expect("mode");
  let path = format!("{}:/usr/bin:/bin", bin.display());
  let env = Env::from_pairs([("HOME", "/tmp"), ("PATH", path.as_str())]);
  let mut graph = graph_248();
  graph.clone_root = tmp.path().join("clones").display().to_string();
  graph.state_dir = tmp.path().join("state").display().to_string();
  let issue = graph.find("homebrew-tap-1").expect("issue").clone();
  let place = || {
    let mut log = Vec::new();
    super::clone_checkout(&graph, &issue, false, &mut log, &env).map_err(|err| err.message)
  };
  let (first, second) = std::thread::scope(|scope| {
    let a = scope.spawn(place);
    let b = scope.spawn(place);
    (a.join().expect("a"), b.join().expect("b"))
  });
  assert_eq!(first, second);
  assert!(first.is_ok(), "{first:?}");
  let logged = std::fs::read_to_string(&calls).expect("calls");
  assert_eq!(logged.lines().count(), 1, "{logged}");
}
