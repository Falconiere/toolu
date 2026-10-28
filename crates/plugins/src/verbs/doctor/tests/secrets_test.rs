use std::collections::BTreeSet;
use std::os::unix::fs::PermissionsExt as _;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use super::{NEEDS, check};
use crate::verbs::doctor::{Level, Scope};

fn roots(home: &std::path::Path) -> Roots {
  let env = Env::from_pairs([("HOME", home.display().to_string())]);
  Roots::new(env, Some(Host::Claude))
}

#[test]
fn missing_env_keys_warn_once_per_variable() {
  let home = tempfile::tempdir().unwrap();
  let roots = roots(home.path());
  let installed = BTreeSet::from([
    "jev".to_owned(),
    "statusline".to_owned(),
    "code-review".to_owned(),
  ]);
  let findings = check(&roots, roots.env(), &installed);
  let keys: Vec<_> = findings
    .iter()
    .filter(|finding| finding.scope == Scope::Secrets && finding.status == Level::Warn)
    .filter_map(|finding| finding.name.as_deref())
    .collect();
  assert!(keys.contains(&"TYPESAFE_API_KEY"));
  assert!(keys.contains(&"OPENROUTER_API_KEY"));
  assert_eq!(
    keys
      .iter()
      .filter(|key| **key == "TYPESAFE_API_KEY")
      .count(),
    1
  );
  assert!(NEEDS.iter().any(|need| need.plugin == "jev"));
}

#[test]
fn unsafe_secrets_json_permissions_fail() {
  let home = tempfile::tempdir().unwrap();
  let dir = home.path().join(".claude/toolu");
  std::fs::create_dir_all(&dir).unwrap();
  let path = dir.join("secrets.json");
  std::fs::write(&path, r#"{"status_token":"canary"}"#).unwrap();
  std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
  let roots = roots(home.path());
  let findings = check(&roots, roots.env(), &BTreeSet::new());
  assert!(
    findings.iter().any(
      |finding| finding.status == Level::Fail && finding.summary.contains("unsafe permissions")
    ),
    "{findings:?}"
  );
}
