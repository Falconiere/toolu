use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use super::{Backend, DEFAULT_NAME, Resolved, Settings, parse, resolve};
use crate::facts::{Facts, Os};

const HOME: &str = "/home/dev";

fn section(value: &Value) -> Map<String, Value> {
  value.as_object().unwrap().clone()
}

fn parsed(value: &Value) -> Result<Settings, String> {
  parse(Some(&section(value)), Path::new(HOME))
}

fn facts(os: Os, cores: u32, memory_gib: u32) -> Facts {
  Facts {
    os,
    cores,
    memory_gib,
  }
}

#[test]
fn every_setting_is_read_and_tilde_means_home() {
  let settings = parsed(&json!({
    "backend": "lima",
    "name": "work-vm2",
    "cpus": 6,
    "memory": 12,
    "disk": 60,
    "template": "template://ubuntu-24.04",
    "mounts": ["~", "~/.herdr/worktrees", "/opt/src"],
  }))
  .unwrap();
  let resolved = resolve(settings, &facts(Os::Linux, 2, 4));
  assert_eq!(
    resolved,
    Resolved {
      backend: Backend::Lima,
      name: "work-vm2".to_owned(),
      cpus: 6,
      memory_gib: 12,
      disk_gib: 60,
      template: Some("template://ubuntu-24.04".to_owned()),
      mounts: vec![
        PathBuf::from(HOME),
        PathBuf::from("/home/dev/.herdr/worktrees"),
        PathBuf::from("/opt/src"),
      ],
    }
  );
}

#[test]
fn defaults_follow_the_machine() {
  let none = parse(None, Path::new(HOME)).unwrap();
  assert_eq!(none, Settings::default());
  let mac = resolve(none.clone(), &facts(Os::Macos, 12, 32));
  assert_eq!(
    (mac.backend, mac.cpus, mac.memory_gib, mac.disk_gib),
    (Backend::Lima, 10, 16, 100)
  );
  assert_eq!(mac.name, DEFAULT_NAME);
  let small = resolve(none.clone(), &facts(Os::Linux, 2, 4));
  assert_eq!(
    (small.backend, small.cpus, small.memory_gib),
    (Backend::Host, 2, 2)
  );
  let big = resolve(none, &facts(Os::Other, 64, 256));
  assert_eq!(
    (big.backend, big.cpus, big.memory_gib),
    (Backend::Host, 62, 32)
  );
  let null = parsed(&json!({ "cpus": null, "mounts": null })).unwrap();
  assert_eq!(null, Settings::default());
}

#[test]
fn an_unknown_key_is_named_with_the_known_ones() {
  let err = parsed(&json!({ "backend": "host", "cpu": 4 })).unwrap_err();
  assert_eq!(
    err,
    "sandbox.cpu is not a setting; the settings are backend, name, cpus, memory, disk, template, mounts"
  );
}

#[test]
fn each_invalid_value_names_its_setting_and_what_it_must_be() {
  let cases = [
    (
      json!({ "backend": "docker" }),
      "sandbox.backend must be \"lima\" or \"host\", not \"docker\"",
    ),
    (
      json!({ "name": "Bad_Name" }),
      "sandbox.name must be 1 to 63 lowercase letters",
    ),
    (json!({ "name": "-vm" }), "sandbox.name must be"),
    (json!({ "name": "a".repeat(64) }), "sandbox.name must be"),
    (
      json!({ "cpus": 0 }),
      "sandbox.cpus must be a whole number from 1 to 256, not 0",
    ),
    (json!({ "cpus": 2.5 }), "sandbox.cpus must be"),
    (
      json!({ "memory": "8g" }),
      "sandbox.memory must be whole GiB from 1 to 1024, not \"8g\"",
    ),
    (
      json!({ "disk": 5 }),
      "sandbox.disk must be whole GiB from 10 to 8192, not 5",
    ),
    (
      json!({ "template": " " }),
      "sandbox.template must be a non-empty string",
    ),
    (
      json!({ "mounts": "~/x" }),
      "sandbox.mounts must be a list of absolute paths",
    ),
    (
      json!({ "mounts": ["relative/dir"] }),
      "sandbox.mounts must be a list of absolute paths or paths starting with ~/, not \"relative/dir\"",
    ),
    (json!({ "mounts": [3] }), "sandbox.mounts must be"),
  ];
  for (value, start) in cases {
    let err = parsed(&value).unwrap_err();
    assert!(err.starts_with(start), "{value}: {err}");
  }
}
