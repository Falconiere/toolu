use std::path::PathBuf;

use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_runtime::registry::{ModuleKind, RegistryEvent};

use super::{Manifests, Read};
use crate::registry::Entry;

struct Named;

impl Rule for Named {
  fn spec(&self) -> &'static str {
    "x@t"
  }
  fn name(&self) -> &'static str {
    "r"
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPre
  }
  fn run(&self, _event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> Decision {
    Decision::Allow
  }
}

fn entry(path: PathBuf, kind: ModuleKind) -> Entry {
  let file = path.file_name().unwrap().to_string_lossy().into_owned();
  let (spec, rest) = file.split_once("__").unwrap();
  Entry {
    spec: spec.to_owned(),
    name: rest.split('.').next().unwrap().to_owned(),
    file: file.clone(),
    path,
    kind,
  }
}

#[test]
fn manifests_are_read_once_with_their_rule_and_problems_lose_the_path() {
  let dir = tempfile::tempdir().unwrap();
  let body = |spec: &str, name: &str| {
    format!(r#"{{"version":1,"spec":"{spec}","name":"{name}","event":"tool/pre","matcher":"*"}}"#)
  };
  std::fs::write(dir.path().join("x@t__r.json"), body("x@t", "r")).unwrap();
  std::fs::write(dir.path().join("y@t__s.json"), body("y@t", "s")).unwrap();
  std::fs::write(dir.path().join("z@t__bad.json"), "{").unwrap();
  let entries: Vec<Entry> = ["x@t__r.json", "y@t__s.json", "z@t__bad.json", "x@t__m.sh"]
    .iter()
    .map(|file| {
      let kind = if std::path::Path::new(file).extension() == Some("sh".as_ref()) {
        ModuleKind::Bash
      } else {
        ModuleKind::Manifest
      };
      entry(dir.path().join(file), kind)
    })
    .collect();
  let manifests = Manifests::read(&entries, RegistryEvent::ToolPre, &[&Named]);
  assert!(matches!(
    manifests.get("x@t__r.json"),
    Some(Read::Valid(_, Some(_)))
  ));
  assert!(matches!(
    manifests.get("y@t__s.json"),
    Some(Read::Valid(_, None))
  ));
  match manifests.get("z@t__bad.json") {
    Some(Read::Problem(reason)) => assert!(reason.starts_with("EOF while parsing"), "{reason}"),
    _ => panic!("a bad manifest is a problem"),
  }
  assert!(manifests.get("x@t__m.sh").is_none());
  assert_eq!(
    manifests.usable_specs().into_iter().collect::<Vec<_>>(),
    ["x@t"]
  );
}
