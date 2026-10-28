//! Registration publishes the `semantic-git` and `commit-note` manifests,
//! idempotently.

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::register;

#[test]
fn register_publishes_both_manifests_once() {
  let dir = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("TOOLU_CONFIG_DIR", dir.path().display().to_string()),
  ]);
  let outcome = register(&env, Host::Claude);
  assert_eq!(outcome.stdout, None);
  let manifest = dir
    .path()
    .join("toolu/pre-tools.d/git-better@toolu__semantic-git.json");
  let body = std::fs::read_to_string(&manifest).unwrap();
  assert_eq!(
    body,
    "{\"version\":1,\"spec\":\"git-better@toolu\",\"name\":\"semantic-git\",\"event\":\"tool/pre\",\"matcher\":\"Bash|Shell\"}\n"
  );
  let post = dir
    .path()
    .join("toolu/post-tools.d/git-better@toolu__commit-note.json");
  assert_eq!(
    std::fs::read_to_string(&post).unwrap(),
    "{\"version\":1,\"spec\":\"git-better@toolu\",\"name\":\"commit-note\",\"event\":\"tool/post\",\"matcher\":\"Bash|Shell\"}\n"
  );
  assert_eq!(register(&env, Host::Claude).stdout, None);
  assert_eq!(std::fs::read_to_string(&manifest).unwrap(), body);
}

#[test]
fn a_failed_registration_is_a_notice_not_a_failure() {
  let dir = tempfile::tempdir().unwrap();
  let blocker = dir.path().join("toolu");
  std::fs::write(&blocker, "a file where the registry directory goes").unwrap();
  let env = Env::from_pairs([
    ("HOME", dir.path().display().to_string()),
    ("TOOLU_CONFIG_DIR", dir.path().display().to_string()),
  ]);
  let outcome = register(&env, Host::Claude);
  assert_eq!(outcome.exit, toolu_protocol::exit::Exit::Success);
  let notice = outcome.stdout.unwrap();
  assert!(
    notice.contains("git-better registration failed"),
    "{notice}"
  );
}
