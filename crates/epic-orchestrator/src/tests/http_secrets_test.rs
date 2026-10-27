use std::os::unix::fs::PermissionsExt as _;
use std::thread;
use std::time::Duration;

use toolu_runtime::config::secrets;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::LiveSecrets;

fn token(roots: &Roots) -> String {
  secrets::load(roots)
    .expect("load")
    .status_token()
    .expect("token")
    .to_owned()
}

fn served(live: &LiveSecrets) -> String {
  live.current().status_token().unwrap_or_default().to_owned()
}

#[test]
fn a_rotated_token_is_served_after_the_check_window_and_a_bad_file_keeps_the_last_good() {
  let temp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", temp.path().display().to_string())]);
  let roots = Roots::new(env, None);
  let path = secrets::rotate_status_token(&roots).expect("first token");
  let first = token(&roots);
  let live = LiveSecrets::new(secrets::load(&roots).expect("load"), Some(roots.clone()));
  assert_eq!(served(&live), first);

  secrets::rotate_status_token(&roots).expect("second token");
  let second = token(&roots);
  assert_ne!(first, second);
  assert_eq!(served(&live), first, "inside the check window");
  thread::sleep(Duration::from_millis(1_100));
  assert_eq!(served(&live), second);

  thread::sleep(Duration::from_millis(20));
  secrets::rotate_status_token(&roots).expect("third token");
  let third = token(&roots);
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).expect("unsafe");
  thread::sleep(Duration::from_millis(1_100));
  assert_eq!(
    served(&live),
    second,
    "an unsafe file keeps the last good token"
  );
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).expect("safe");
  thread::sleep(Duration::from_millis(1_100));
  assert_eq!(served(&live), third, "the failed read is retried");
}

#[test]
fn fixed_secrets_never_reload() {
  let temp = tempfile::tempdir().expect("temp");
  let env = Env::from_pairs([("TOOLU_CONFIG_DIR", temp.path().display().to_string())]);
  let roots = Roots::new(env, None);
  secrets::rotate_status_token(&roots).expect("token");
  let live = LiveSecrets::new(secrets::Secrets::none(), None);
  thread::sleep(Duration::from_millis(1_100));
  assert!(live.current().status_token().is_none());
}
