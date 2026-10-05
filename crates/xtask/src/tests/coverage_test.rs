use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::{Export, floor, judge};
use crate::data::{self, CoverageFloor};
use crate::workspace::{Member, Workspace};

const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

fn export(files: &[(&str, u64, u64)]) -> Export {
  let files: Vec<String> = files
    .iter()
    .map(|(name, covered, count)| {
      format!("{{\"filename\":\"{name}\",\"summary\":{{\"lines\":{{\"count\":{count},\"covered\":{covered},\"percent\":0}}}}}}")
    })
    .collect();
  serde_json::from_str(&format!(
    "{{\"data\":[{{\"files\":[{}]}}],\"type\":\"llvm.coverage.json.export\"}}",
    files.join(",")
  ))
  .unwrap()
}

fn workspace() -> Workspace {
  let member = |name: &str, dir: &str| Member {
    name: name.to_owned(),
    dir: PathBuf::from(dir),
    is_lib: true,
  };
  Workspace {
    root: PathBuf::from("/r"),
    members: vec![
      member("toolu-engine", "crates/core/engine"),
      member("demo", "crates/demo"),
      member("empty", "crates/empty"),
    ],
    files: Vec::new(),
  }
}

#[test]
fn crates_under_their_floor_are_reported_and_tests_are_not_counted() {
  let rules = data::rules(Path::new(REPO)).unwrap();
  let floors = CoverageFloor {
    floors: BTreeMap::from([("demo".to_owned(), 95.0)]),
  };
  let export = export(&[
    ("/r/crates/core/engine/src/lib.rs", 89, 100),
    ("/r/crates/demo/src/lib.rs", 96, 100),
    ("/r/crates/demo/src/tests/lib_test.rs", 0, 100),
    ("/r/crates/demo/tests/it.rs", 0, 100),
    ("/r/crates/empty/src/lib.rs", 0, 0),
    ("/elsewhere/lib.rs", 0, 10),
  ]);
  assert_eq!(
    judge(&workspace(), &export, &rules, &floors),
    ["coverage toolu-engine: 89.0% lines, floor 90% — add tests"]
  );
  let raised = CoverageFloor {
    floors: BTreeMap::from([("demo".to_owned(), 97.0)]),
  };
  assert_eq!(judge(&workspace(), &export, &rules, &raised).len(), 2);
}

#[test]
fn the_floor_is_the_highest_of_default_strict_and_row() {
  let rules = data::rules(Path::new(REPO)).unwrap();
  let floors = CoverageFloor {
    floors: BTreeMap::from([("demo".to_owned(), 80.0), ("toolu-shell".to_owned(), 93.0)]),
  };
  assert_eq!(floor(&rules, &floors, "demo"), 85.0);
  assert_eq!(floor(&rules, &floors, "toolu-state"), 90.0);
  assert_eq!(floor(&rules, &floors, "toolu-shell"), 93.0);
}
