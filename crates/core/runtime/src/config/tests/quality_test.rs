use std::path::Path;

use serde_json::{Value, json};
use toolu_protocol::host::Host;

use super::{
  QualityLang, ThresholdSource, native_max_lines, quality_flag, quality_threshold,
  ts_max_file_lines_resolved,
};
use crate::config::load::LoadedConfig;

fn config(data: Value) -> LoadedConfig {
  let Value::Object(map) = data else {
    panic!("not an object")
  };
  LoadedConfig::from_data(map, Host::Claude)
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  for (name, text) in files {
    std::fs::write(dir.path().join(name), text).unwrap();
  }
  dir
}

#[test]
fn positive_numbers_and_numeric_strings_override() {
  let c = config(json!({ "lang": {
    "ts": { "maxFileLines": " 120 ", "maxFnLines": 0 },
    "rust": { "maxFileLines": -5, "maxFnLines": 45.9, "maxImplLines": "2.5e2" },
    "python": { "maxFileLines": "off", "maxFnLines": true }
  } }));
  let empty = project(&[]);
  let root = Some(empty.path());
  let got = [
    quality_threshold(&c, QualityLang::Ts, "maxFileLines", root),
    quality_threshold(&c, QualityLang::Ts, "maxFnLines", root),
    quality_threshold(&c, QualityLang::Rust, "maxFileLines", root),
    quality_threshold(&c, QualityLang::Rust, "maxFnLines", root),
    quality_threshold(&c, QualityLang::Rust, "maxImplLines", root),
    quality_threshold(&c, QualityLang::Python, "maxFileLines", root),
    quality_threshold(&c, QualityLang::Python, "maxFnLines", root),
    quality_threshold(&c, QualityLang::Python, "noSuchKey", root),
  ];
  assert_eq!(got, [120, 60, 500, 45, 250, 400, 50, 0]);
}

#[test]
fn a_sub_one_override_floors_to_zero_and_a_huge_one_saturates() {
  let c = config(json!({ "lang": { "rust": { "maxFnLines": 0.5, "maxFileLines": "1e999" } } }));
  assert_eq!(
    quality_threshold(&c, QualityLang::Rust, "maxFnLines", None),
    0
  );
  assert_eq!(
    quality_threshold(&c, QualityLang::Rust, "maxFileLines", None),
    u64::MAX
  );
}

#[test]
fn native_max_lines_reads_every_eslint_and_oxlint_encoding() {
  let rules = |rule: Value| json!({ "rules": { "max-lines": rule } });
  assert_eq!(native_max_lines(&rules(json!(150))), Some(150));
  assert_eq!(native_max_lines(&rules(json!(["error", 250]))), Some(250));
  assert_eq!(
    native_max_lines(&rules(
      json!(["warn", { "max": "200", "skipBlankLines": true }])
    )),
    Some(200)
  );
  assert_eq!(native_max_lines(&rules(json!([2, 90.7]))), Some(90));
  for rule in [
    json!(["off", 100]),
    json!([0, 100]),
    json!(["error"]),
    json!("error"),
    json!(["error", { "max": 0 }]),
    json!({ "max": 5 }),
  ] {
    assert_eq!(native_max_lines(&rules(rule.clone())), None, "{rule}");
  }
  assert_eq!(native_max_lines(&json!([rules(json!(1))])), None);
}

#[test]
fn the_active_linter_decides_oxc_over_eslint() {
  let both = project(&[
    (".oxlintrc.json", r#"{"rules":{"max-lines":["error",222]}}"#),
    (".eslintrc.json", r#"{"rules":{"max-lines":111}}"#),
  ]);
  let empty = config(json!({}));
  assert_eq!(
    ts_max_file_lines_resolved(&empty, Some(both.path())),
    (222, ThresholdSource::Native)
  );
  assert_eq!(
    quality_threshold(&empty, QualityLang::Ts, "maxFileLines", Some(both.path())),
    222
  );
  assert_eq!(
    quality_threshold(&empty, QualityLang::Ts, "maxFnLines", Some(both.path())),
    60
  );
  let eslint = project(&[(
    ".eslintrc.json",
    r#"{"rules":{"max-lines":["error",{"max":180}]}}"#,
  )]);
  assert_eq!(
    ts_max_file_lines_resolved(&empty, Some(eslint.path())),
    (180, ThresholdSource::Native)
  );
}

#[test]
fn biome_flat_eslint_and_broken_configs_fall_through_to_the_default() {
  let empty = config(json!({}));
  let cases = [
    project(&[
      ("biome.json", "{}"),
      (".oxlintrc.json", r#"{"rules":{"max-lines":99}}"#),
    ]),
    project(&[("eslint.config.js", "export default []")]),
    project(&[(".oxlintrc.json", "{ not json")]),
  ];
  for dir in &cases {
    assert_eq!(
      ts_max_file_lines_resolved(&empty, Some(dir.path())),
      (300, ThresholdSource::Default)
    );
  }
  let missing = Path::new("/nonexistent/toolu-project");
  assert_eq!(
    ts_max_file_lines_resolved(&empty, Some(missing)).1,
    ThresholdSource::Default
  );
}

#[test]
fn an_override_beats_the_native_layer_and_no_root_reads_this_repository() {
  let native = project(&[(".oxlintrc.json", r#"{"rules":{"max-lines":222}}"#)]);
  let c = config(json!({ "lang": { "ts": { "maxFileLines": 80 } } }));
  assert_eq!(
    ts_max_file_lines_resolved(&c, Some(native.path())),
    (80, ThresholdSource::Override)
  );
  // This repository's .oxlintrc.json sets no max-lines rule.
  assert_eq!(
    ts_max_file_lines_resolved(&config(json!({})), None),
    (300, ThresholdSource::Default)
  );
}

#[test]
fn quality_flag_honors_only_a_json_boolean() {
  let c = config(json!({ "lang": { "ts": { "noMocks": false }, "rust": { "noMocks": "false" } } }));
  assert!(!quality_flag(&c, QualityLang::Ts, "noMocks", true));
  assert!(quality_flag(&c, QualityLang::Rust, "noMocks", true));
  assert!(quality_flag(&c, QualityLang::Python, "noMocks", true));
}
