use std::path::PathBuf;

use super::{
  ModuleKind, NameParse, ParsedName, RegistryEvent, event_dir, file_name, parse_name, registry_root,
};
use crate::env::Env;
use crate::host::roots::Roots;

#[test]
fn toolu_config_dir_wins_on_codex() {
  let base = [
    ("HOME", "/home/u"),
    ("CODEX_HOME", "/cx"),
    ("PLUGIN_ROOT", "/p"),
  ];
  let codex = Roots::new(Env::from_pairs(base), None);
  assert_eq!(
    event_dir(&codex, RegistryEvent::ToolPre),
    PathBuf::from("/cx/toolu/pre-tools.d")
  );
  let toolu = Roots::new(Env::from_pairs(base).with("TOOLU_CONFIG_DIR", "/t"), None);
  assert_eq!(registry_root(&toolu), PathBuf::from("/t/toolu"));
  assert_eq!(
    event_dir(&toolu, RegistryEvent::ToolPre),
    PathBuf::from("/t/toolu/pre-tools.d")
  );
  assert_eq!(
    event_dir(&toolu, RegistryEvent::ToolPost),
    PathBuf::from("/t/toolu/post-tools.d")
  );
}

#[test]
fn events_name_their_slug_and_directory() {
  let named: Vec<(&str, &str)> = RegistryEvent::ALL
    .iter()
    .map(|event| (event.slug(), event.dir_name()))
    .collect();
  assert_eq!(
    named,
    [("tool/pre", "pre-tools.d"), ("tool/post", "post-tools.d")]
  );
  assert_eq!(
    serde_json::to_string(&RegistryEvent::ToolPost).unwrap(),
    "\"tool/post\""
  );
}

#[test]
fn file_names_round_trip_through_parse_name() {
  for kind in [ModuleKind::Esm, ModuleKind::Bash, ModuleKind::Manifest] {
    let base = file_name("ts-quality@toolu", "max.lines", kind).unwrap();
    let parsed = ParsedName {
      spec: "ts-quality@toolu".to_owned(),
      name: "max.lines".to_owned(),
      kind,
    };
    assert_eq!(parse_name(&base), NameParse::Module(parsed));
  }
  assert_eq!(
    file_name("a@b", "c", ModuleKind::Manifest).unwrap(),
    "a@b__c.json"
  );
}

#[test]
fn a_spec_or_name_that_cannot_round_trip_is_refused() {
  for spec in ["", "a b", "a/b", "a__b"] {
    let error = file_name(spec, "x", ModuleKind::Esm).unwrap_err().0;
    assert!(
      error.starts_with("registry: invalid plugin spec"),
      "{spec:?}: {error}"
    );
  }
  for name in ["", "a/b", ".hidden"] {
    let error = file_name("a@b", name, ModuleKind::Esm).unwrap_err().0;
    assert!(
      error.starts_with("registry: invalid module name"),
      "{name:?}: {error}"
    );
  }
}

#[test]
fn other_files_and_unnamespaced_modules_are_told_apart() {
  for base in ["README.md", "a__b.ts", "json", "a__b."] {
    assert_eq!(parse_name(base), NameParse::NotModule, "{base}");
  }
  for base in ["plain.js", "__x.sh", "a b__c.json"] {
    assert_eq!(parse_name(base), NameParse::Unnamespaced, "{base}");
  }
  let empty_name = ParsedName {
    spec: "a".to_owned(),
    name: String::new(),
    kind: ModuleKind::Esm,
  };
  assert_eq!(parse_name("a__.js"), NameParse::Module(empty_name));
}
