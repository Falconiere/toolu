use std::path::{Path, PathBuf};

use serde_json::json;

use super::{payload_stdin, run, runs};
use crate::options::Options;

#[test]
fn the_project_is_spliced_as_a_json_escaped_path() {
  let payload =
    json!({"stdin": {"cwd": "${PROJECT}", "tool_input": {"file_path": "${PROJECT}/a.ts"}}});
  let text = payload_stdin(Some(&payload), Path::new("/tmp/a \"b\"")).unwrap();
  let doc: serde_json::Value = serde_json::from_str(&text).unwrap();
  assert_eq!(doc["cwd"], "/tmp/a \"b\"");
  assert_eq!(doc["tool_input"]["file_path"], "/tmp/a \"b\"/a.ts");
  assert!(payload_stdin(Some(&json!({})), Path::new("/p")).is_err());
}

#[test]
fn runs_default_to_ten_and_refuse_zero() {
  let mut options = Options::default();
  assert_eq!(runs(&options).unwrap(), 10);
  options.runs = Some("0".to_owned());
  assert_eq!(
    runs(&options).unwrap_err(),
    "--runs takes a whole number above 0, got 0"
  );
}

#[test]
fn a_missing_toolu_is_a_setup_error() {
  let options = Options {
    root: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../.."),
    bin: Some(PathBuf::from("/no/such/toolu")),
    ..Options::default()
  };
  let err = run(&options).unwrap_err();
  assert!(
    err.starts_with("bench-hooks: no toolu at /no/such/toolu"),
    "{err}"
  );
}
