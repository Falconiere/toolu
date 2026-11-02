use super::*;
use crate::delegation::machines_command;

#[test]
fn explicit_recipe_updates_are_persistent_and_invalid_argv_is_rejected() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  let path = dir.path().join("recipe.json");
  std::fs::write(&path, r#"[["cargo","fetch"]]"#).unwrap();
  let args = machines_command()
    .try_get_matches_from([
      "machines",
      "recipe",
      "--origin",
      "origin",
      "--file",
      path.to_str().unwrap(),
    ])
    .unwrap();
  setup_machines(&args, &env).unwrap();
  assert_eq!(
    config::load(&config::root(&env)).unwrap().recipes["origin"],
    vec![vec!["cargo", "fetch"]]
  );
  std::fs::write(&path, "[[]]").unwrap();
  assert!(setup_machines(&args, &env).is_err());
  assert_eq!(
    config::load(&config::root(&env)).unwrap().recipes["origin"],
    vec![vec!["cargo", "fetch"]]
  );
}

#[test]
fn repeating_registration_preserves_drain_for_idle_and_occupied_workers() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", dir.path().to_str().unwrap())]);
  let register = machines_command()
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
  setup_machines(&register, &env).unwrap();
  let drain = super::super::operations_command()
    .try_get_matches_from(["machines", "drain", "worker"])
    .unwrap();
  machine_operation(&drain, &env).unwrap();
  setup_machines(&register, &env).unwrap();
  let root = config::root(&env);
  assert!(config::load(&root).unwrap().machines["worker"].draining);
  let resume = super::super::operations_command()
    .try_get_matches_from(["machines", "resume", "worker"])
    .unwrap();
  machine_operation(&resume, &env).unwrap();
  let store = Store::new(&root);
  let request = super::super::Submission {
    key: "task".into(),
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
    .reserve("task", &config::load(&root).unwrap(), 0)
    .unwrap()
    .unwrap();
  machine_operation(&drain, &env).unwrap();
  setup_machines(&register, &env).unwrap();
  assert!(config::load(&root).unwrap().machines["worker"].draining);
}
