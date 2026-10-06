use std::path::Path;

use super::check_layers;
use crate::data::{self, CapabilityCrates};
use crate::layers::LayerTable;
use crate::metadata;

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn table() -> LayerTable {
  let text =
    std::fs::read_to_string(Path::new(REPO).join(data::DATA_DIR).join("layers.json")).unwrap();
  LayerTable::parse(&text).unwrap()
}

#[test]
fn the_repository_has_no_violation() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  let rules = data::rules(Path::new(REPO)).unwrap();
  let report = check_layers(&metadata, &table(), &rules.capability_crates);
  assert_eq!(
    report.violations,
    Vec::<String>::new(),
    "{:?}",
    report.violations
  );
  assert_eq!(report.crates, 20);
}

#[test]
fn a_capability_crate_linked_outside_its_owner_is_named() {
  let metadata = metadata::load(Path::new(REPO)).unwrap();
  let owned = [CapabilityCrates {
    id: "json".to_owned(),
    crates: vec!["serde_json".to_owned()],
    owner: "toolu-http".to_owned(),
  }];
  let report = check_layers(&metadata, &table(), &owned);
  assert_eq!(
    report.violations,
    [
      "crates/core/runtime (toolu-runtime) depends on serde_json: only toolu-http may link a \
       `json` crate",
      "crates/cli (toolu-cli) depends on serde_json: only toolu-http may link a `json` crate",
      "crates/xtask (xtask) depends on serde_json: only toolu-http may link a `json` crate",
    ]
  );
}
