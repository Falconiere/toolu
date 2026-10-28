use std::path::PathBuf;

use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::env::Env;

use super::{setup, status, vm};
use crate::context::Context;
use crate::facts::{Facts, Os};
use crate::project::Project;
use crate::settings::{Backend, Resolved};

fn context(root: Option<&str>) -> Context {
  Context {
    env: Env::from_pairs([("HOME", "/home/dev")]),
    facts: Facts {
      os: Os::Linux,
      cores: 4,
      memory_gib: 8,
    },
    settings: Resolved {
      backend: Backend::Host,
      name: "toolu-sandbox".to_owned(),
      cpus: 2,
      memory_gib: 4,
      disk_gib: 100,
      template: None,
      mounts: vec![PathBuf::from("/srv")],
    },
    project: Project {
      cwd: PathBuf::from("/repo/sub"),
      root: root.map(PathBuf::from),
      common: None,
    },
  }
}

#[test]
fn status_names_the_backend_state_settings_and_project() {
  let report = status(&context(Some("/repo")), "host", None);
  assert_eq!(report.exit, Exit::Success);
  assert_eq!(
    report.doc,
    json!({
      "namespace": "sandbox",
      "verb": "status",
      "backend": "host",
      "state": "host",
      "reason": null,
      "settings": {
        "name": "toolu-sandbox",
        "cpus": 2,
        "memory_gib": 4,
        "disk_gib": 100,
        "template": null,
        "mounts": ["/srv"],
      },
      "project": { "cwd": "/repo/sub", "root": "/repo" },
    })
  );
  assert_eq!(
    report.text,
    "sandbox: host backend, host\n  settings: 2 cpus, 4 GiB memory, 100 GiB disk, VM name toolu-sandbox\n  project: /repo"
  );
  let away = status(&context(None), "unavailable", Some("not built"));
  assert_eq!(away.doc["reason"], "not built");
  assert!(
    away
      .text
      .starts_with("sandbox: host backend, unavailable: not built\n")
  );
  assert!(away.text.ends_with("project: not a git checkout"));
}

#[test]
fn setup_lists_what_is_missing_and_the_four_agent_clis() {
  let ready = setup(&context(None), "host", &[]);
  assert_eq!(ready.exit, Exit::Success);
  assert_eq!(ready.doc["missing"], json!([]));
  let clis: Vec<&str> = ready.doc["next"]
    .as_array()
    .unwrap()
    .iter()
    .map(|vendor| vendor["cli"].as_str().unwrap())
    .collect();
  assert_eq!(clis, ["claude", "codex", "cursor-agent", "muse"]);
  assert!(ready.text.starts_with("sandbox: host backend ready\n"));
  assert!(
    ready
      .text
      .contains("\n  muse: curl -fsSL https://dev.meta.ai/install.sh | bash; then muse login")
  );
  let short = setup(&context(None), "host", &["bwrap", "node"]);
  assert_eq!(short.exit, Exit::Unavailable);
  assert_eq!(short.doc["missing"], json!(["bwrap", "node"]));
  assert!(
    short
      .text
      .starts_with("sandbox: missing on this machine: bwrap, node\n")
  );
}

#[test]
fn a_vm_verb_says_what_changed() {
  let report = vm(&context(None), "stop", "host", false, "nothing to stop");
  assert_eq!(
    report.doc,
    json!({ "namespace": "sandbox", "verb": "stop", "backend": "host", "state": "host", "changed": false })
  );
  assert_eq!(report.text, "sandbox: nothing to stop");
}
