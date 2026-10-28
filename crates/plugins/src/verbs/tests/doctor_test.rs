use std::os::unix::fs::PermissionsExt as _;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::{Counts, Finding, Level, Scope, redact_text_all};

#[test]
fn counts_sum_every_level() {
  let findings = [
    Finding::ok(Scope::Plugin, Some("jev".into()), "ok"),
    Finding::warn(
      Scope::Secrets,
      Some("TYPESAFE_API_KEY".into()),
      "unset",
      None,
    ),
    Finding::fail(Scope::Secrets, Some("secrets.json".into()), "bad", None),
  ];
  let mut counts = Counts::default();
  for finding in &findings {
    counts.add(finding.status);
  }
  assert_eq!(counts.line(), "1 ok, 1 warn, 1 fail");
  assert_eq!(Level::Ok.name(), "ok");
}

#[test]
fn redaction_covers_every_selected_host() {
  let home = tempfile::tempdir().unwrap();
  let env = Env::from_pairs([("HOME", home.path().display().to_string())]);
  for (host, token) in [
    (Host::Claude, "CLAUDE_CANARY_token"),
    (Host::Muse, "MUSE_CANARY_token"),
  ] {
    let roots = toolu_runtime::host::roots::Roots::new(env.clone(), Some(host));
    let path = toolu_runtime::config::secrets::path(&roots);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
      &path,
      format!(r#"{{"version":1,"status_token":"{token}"}}"#),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
      toolu_runtime::config::secrets::load(&roots)
        .unwrap()
        .status_token(),
      Some(token),
      "{host:?} {}",
      path.display()
    );
  }
  let text = redact_text_all(
    "seen CLAUDE_CANARY_token and MUSE_CANARY_token",
    &env,
    &[Host::Claude, Host::Muse],
  );
  assert!(!text.contains("CLAUDE_CANARY_token"), "{text}");
  assert!(!text.contains("MUSE_CANARY_token"), "{text}");
  assert!(text.contains("<redacted>"), "{text}");
}
