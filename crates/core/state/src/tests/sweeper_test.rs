use std::path::Path;

use serde_json::json;
use toolu_protocol::host::Host;
use toolu_runtime::config::load::LoadedConfig;

use super::{glob_files, has_live_violation, positive_gate, slug_of_state_file};

#[test]
fn state_files_belong_to_their_branch_slug() {
  for (name, slug) in [
    ("feat_x.json", "feat_x"),
    ("feat_x.waiver.json", "feat_x"),
    ("feat_x.pending-waiver.json", "feat_x"),
    ("main.waiver.pending-waiver.json", "main"),
    ("plain", "plain"),
  ] {
    assert_eq!(
      slug_of_state_file(Path::new("/s").join(name).as_path()),
      slug,
      "{name}"
    );
  }
}

#[test]
fn gate_thresholds_are_positive_numbers_floored() {
  let config = |gates: serde_json::Value| {
    let data = json!({ "gates": gates }).as_object().unwrap().clone();
    LoadedConfig::from_data(data, Host::Claude)
  };
  assert_eq!(
    positive_gate(
      &config(json!({ "stateTtlHours": 2.9 })),
      "stateTtlHours",
      24
    ),
    2
  );
  for value in [json!(0), json!(-1), json!("3"), json!(null)] {
    assert_eq!(
      positive_gate(
        &config(json!({ "stateTtlHours": value })),
        "stateTtlHours",
        24
      ),
      24
    );
  }
  assert_eq!(
    positive_gate(&config(json!("not an object")), "stateTtlHours", 24),
    24
  );
}

#[test]
fn globs_list_visible_regular_files_with_the_suffix() {
  let dir = tempfile::tempdir().unwrap();
  for name in ["b.json", "a.json", ".hidden.json", "c.txt"] {
    std::fs::write(dir.path().join(name), "").unwrap();
  }
  std::fs::create_dir(dir.path().join("d.json")).unwrap();
  let names: Vec<String> = glob_files(dir.path(), ".json")
    .iter()
    .map(|file| file.file_name().unwrap().to_string_lossy().into_owned())
    .collect();
  assert_eq!(names, ["a.json", "b.json"]);
  assert_eq!(
    glob_files(&dir.path().join("missing"), ".json"),
    Vec::<std::path::PathBuf>::new()
  );
}

#[test]
fn global_and_existing_files_are_live_violations() {
  let dir = tempfile::tempdir().unwrap();
  let live = dir.path().join("a.ts");
  std::fs::write(&live, "").unwrap();
  assert!(has_live_violation(&["__global__".to_owned()]));
  assert!(has_live_violation(&[
    "/gone".to_owned(),
    live.display().to_string()
  ]));
  assert!(!has_live_violation(&[String::new(), "/gone".to_owned()]));
}
