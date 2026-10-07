use std::path::Path;

use super::super::checks::Status;
use super::check;
use toolu_protocol::host::Host;
use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

fn roots(home: &Path) -> Roots {
  Roots::new(
    Env::from_pairs([
      ("HOME", home.to_str().unwrap()),
      ("TOOLU_PROJECT_DIR", home.to_str().unwrap()),
    ]),
    Some(Host::Claude),
  )
}

#[test]
fn an_unknown_top_level_key_fails_with_the_loader_message() {
  let home = tempfile::tempdir().unwrap();
  let path = home.path().join(".claude/toolu.config.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, r#"{"bogus":1}"#).unwrap();
  let bound = roots(home.path());
  let secrets = secrets::load(&bound);
  let check = check(&bound, home.path(), &secrets);
  assert_eq!(check.status, Status::Fail);
  let summary = check.summary;
  assert!(
    summary.contains("unknown top-level key 'bogus'"),
    "{summary}"
  );
  assert!(summary.contains(&path.display().to_string()), "{summary}");
  assert!(check.details.get("merged").is_some());
}

#[test]
fn malformed_json_warns_and_is_not_a_failure() {
  let home = tempfile::tempdir().unwrap();
  let path = home.path().join(".claude/toolu.config.json");
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  std::fs::write(&path, "{").unwrap();
  let bound = roots(home.path());
  let secrets = secrets::load(&bound);
  let check = check(&bound, home.path(), &secrets);
  assert_eq!(check.status, Status::Warn);
  assert!(
    check.summary.contains("malformed JSON"),
    "{}",
    check.summary
  );
}

#[test]
fn a_missing_config_is_valid() {
  let home = tempfile::tempdir().unwrap();
  let bound = roots(home.path());
  let secrets = secrets::load(&bound);
  let check = check(&bound, home.path(), &secrets);
  assert_eq!(check.status, Status::Ok);
  assert_eq!(check.details["files"], serde_json::json!([]));
  assert!(check.details.get("merged").is_some());
}
