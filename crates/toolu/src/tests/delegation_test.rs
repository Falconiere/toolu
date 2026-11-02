use super::*;

#[test]
fn registration_is_persistent_and_does_not_enroll_the_controller() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  let command = machines_command();
  let args = command
    .try_get_matches_from([
      "machines",
      "register",
      "worker",
      "--root",
      "/srv/work",
      "--agent",
      "codex",
    ])
    .unwrap();
  setup_machines(&args, &env).unwrap();
  setup_machines(&args, &env).unwrap();
  let saved = config::load(&config::root(&env)).unwrap();
  assert_eq!(saved.machines.len(), 1);
  assert_eq!(saved.machines["worker"].capacity, 1);
  assert_eq!(saved.relay, None);
  let args = machines_command()
    .try_get_matches_from(["machines", "remove", "worker"])
    .unwrap();
  setup_machines(&args, &env).unwrap();
  assert!(
    config::load(&config::root(&env))
      .unwrap()
      .machines
      .is_empty()
  );
}

#[test]
fn active_machine_removal_is_refused_but_draining_is_allowed() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  let args = machines_command()
    .try_get_matches_from([
      "machines",
      "register",
      "worker",
      "--root",
      "/srv/work",
      "--agent",
      "codex",
    ])
    .unwrap();
  setup_machines(&args, &env).unwrap();
  let root = config::root(&env);
  let store = Store::new(&root);
  let request = Submission {
    key: "test".into(),
    origin: "origin".into(),
    base: "a".repeat(40),
    brief: "work".into(),
    agent: "codex".into(),
    argv: vec!["codex".into()],
    machine: None,
    group: None,
  };
  store.submit(request, b"input").unwrap();
  store
    .reserve("test", &config::load(&root).unwrap(), 0)
    .unwrap();
  let args = machines_command()
    .try_get_matches_from(["machines", "remove", "worker"])
    .unwrap();
  assert!(setup_machines(&args, &env).is_err());
  let args = operations_command()
    .try_get_matches_from(["machines", "drain", "worker"])
    .unwrap();
  machine_operation(&args, &env).unwrap();
  assert!(config::load(&root).unwrap().machines["worker"].draining);
}
