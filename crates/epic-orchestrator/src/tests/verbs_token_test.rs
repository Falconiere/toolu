use serde_json::json;
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::dashboard_url;
use crate::disk::write_value;
use crate::lock::Held;
use crate::paths::Paths;

#[test]
fn the_url_needs_a_live_engine_a_listener_and_a_token() {
  let temp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([
    (
      "TOOLU_RESOURCE_HOME",
      temp.path().join("res").display().to_string(),
    ),
    (
      "TOOLU_CONFIG_DIR",
      temp.path().join("config").display().to_string(),
    ),
  ]);
  let paths = Paths::from_env(&env);
  assert_eq!(dashboard_url(&env), Err("no engine is running".to_owned()));

  std::fs::create_dir_all(&paths.root).expect("root");
  let _engine = Held::acquire(&paths.lock(), "engine-busy").expect("lock");
  assert_eq!(
    dashboard_url(&env),
    Err("the engine has no status listener".to_owned())
  );

  // A wildcard bind is reached on loopback, as the attention notice does.
  write_value(&paths.http(), &json!({"addr": "0.0.0.0:7717"})).expect("http.json");
  let missing = dashboard_url(&env).expect_err("no token");
  assert!(missing.starts_with("no status token"), "{missing}");

  secrets::rotate_status_token(&Roots::new(env.clone(), None)).expect("token");
  let token = secrets::load(&Roots::new(env.clone(), None))
    .expect("load")
    .status_token()
    .expect("token")
    .to_owned();
  assert_eq!(
    dashboard_url(&env),
    Ok(format!("http://127.0.0.1:7717/#token={token}"))
  );
}
