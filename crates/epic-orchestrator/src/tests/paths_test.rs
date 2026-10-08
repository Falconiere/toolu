use toolu_runtime::env::Env;

use super::Paths;

#[test]
fn the_resource_root_follows_toolu_resource_home() {
  let env = Env::from_pairs([("TOOLU_RESOURCE_HOME", "/srv/resources")]);
  let paths = Paths::from_env(&env);
  assert_eq!(
    paths.socket(),
    std::path::PathBuf::from("/srv/resources/engine.sock")
  );
  assert_eq!(
    paths.lock(),
    std::path::PathBuf::from("/srv/resources/engine.lock")
  );
  assert_eq!(
    paths.spool(),
    std::path::PathBuf::from("/srv/resources/spool")
  );
  assert_eq!(
    paths.service(),
    std::path::PathBuf::from("/srv/resources/toolu-epic.service")
  );
}
