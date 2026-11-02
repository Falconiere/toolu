use super::{Config, Machine, load, register, root, update, validate};
use toolu_runtime::env::Env;

fn machine() -> Machine {
  Machine {
    name: "worker-1".into(),
    root: "/work/project".into(),
    agents: vec!["codex".into()],
    capacity: 2,
    draining: false,
  }
}

#[test]
fn configuration_root_obeys_environment_precedence() {
  let env = Env::from_pairs([
    ("HOME", "/home/a"),
    ("XDG_CONFIG_HOME", "/xdg"),
    ("TOOLU_CONFIG_DIR", "/custom"),
  ]);
  assert_eq!(root(&env), std::path::PathBuf::from("/custom/delegation"));
  assert_eq!(
    root(&Env::from_pairs([("XDG_CONFIG_HOME", "/xdg")])),
    std::path::PathBuf::from("/xdg/toolu/delegation")
  );
  assert_eq!(
    root(&Env::from_pairs([("HOME", "/home/a")])),
    std::path::PathBuf::from("/home/a/.config/toolu/delegation")
  );
}

#[test]
fn registry_persists_and_unchanged_update_preserves_file() {
  let dir = tempfile::tempdir().unwrap();
  assert_eq!(load(dir.path()).unwrap(), Config::default());
  let saved = update(dir.path(), |config| register(config, machine())).unwrap();
  assert_eq!(load(dir.path()).unwrap(), saved);
  let path = dir.path().join("config.json");
  let before = std::fs::metadata(&path).unwrap().modified().unwrap();
  update(dir.path(), |_| Ok(())).unwrap();
  assert_eq!(std::fs::metadata(path).unwrap().modified().unwrap(), before);
}

#[test]
fn invalid_machines_and_corrupt_files_are_rejected() {
  let mut config = Config::default();
  for invalid in [
    Machine {
      name: "../bad".into(),
      ..machine()
    },
    Machine {
      root: "/a/../b".into(),
      ..machine()
    },
    Machine {
      agents: vec!["unknown".into()],
      ..machine()
    },
    Machine {
      capacity: 0,
      ..machine()
    },
  ] {
    assert!(register(&mut config, invalid).is_err());
  }
  config.version = 2;
  assert!(validate(&config).is_err());
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("config.json"), "broken").unwrap();
  assert!(load(dir.path()).is_err());
}
