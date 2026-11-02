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
