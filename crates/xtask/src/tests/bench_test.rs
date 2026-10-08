use std::path::PathBuf;

use serde_json::Value;

use super::{Request, deterministic, run};
use crate::options::Options;

fn repo() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn request<'a>(
  root: &'a PathBuf,
  queries: &'a PathBuf,
  corpus: &'a PathBuf,
  out_dir: &'a PathBuf,
  api_key: Option<&'a str>,
) -> Request<'a> {
  Request {
    root,
    queries,
    corpus,
    out_dir,
    api_key,
  }
}

#[test]
fn this_repository_writes_bytes_div_4() {
  let root = repo();
  let out = tempfile::tempdir().expect("temp dir");
  let out_dir = out.path().to_path_buf();
  let queries = root.join("benchmarks/cases/retrieval/queries.tsv");
  let path = deterministic(&request(&root, &queries, &root, &out_dir, None)).expect("result");
  let doc: Value =
    serde_json::from_str(&std::fs::read_to_string(&path).expect("json")).expect("parse");
  assert_eq!(doc["tokenizer"]["source"], "bytes-div-4");
  assert_eq!(doc["tokenizer"]["mode"], "heuristic");
  crate::bench_result::validate(&path).expect("valid");
}

#[test]
fn a_set_api_key_writes_nothing() {
  let root = repo();
  let out = tempfile::tempdir().expect("temp dir");
  let out_dir = out.path().to_path_buf();
  let queries = root.join("benchmarks/cases/retrieval/queries.tsv");
  let err = deterministic(&request(&root, &queries, &root, &out_dir, Some("x"))).expect_err("key");
  assert_eq!(err, "bench: exact token counts are not available");
  assert!(
    std::fs::read_dir(out.path())
      .expect("list")
      .next()
      .is_none()
  );
}

#[test]
fn a_missing_target_is_saved_zero() {
  let root = repo();
  let out = tempfile::tempdir().expect("temp dir");
  let out_dir = out.path().to_path_buf();
  let queries = root.join("benchmarks/fixtures/queries.tsv");
  let corpus = root.join("benchmarks/fixtures");
  let path = deterministic(&request(&root, &queries, &corpus, &out_dir, None)).expect("result");
  let doc: Value =
    serde_json::from_str(&std::fs::read_to_string(&path).expect("json")).expect("parse");
  let cases = doc["cases"].as_array().expect("cases");
  let target = cases
    .iter()
    .find(|case| case["id"] == "target")
    .expect("target");
  let missing = cases
    .iter()
    .find(|case| case["id"] == "missing")
    .expect("missing");
  assert!(target["saved"].as_i64().expect("saved") > 0);
  assert_eq!(missing["saved"], 0);
  assert_eq!(missing["note"], "missing-or-empty-target");
  assert!(
    doc["notes"]
      .as_str()
      .unwrap_or("")
      .contains("missing-target")
  );
}

#[test]
fn an_unknown_mode_is_an_error() {
  let err = run(&Options {
    root: repo(),
    files: vec![PathBuf::from("nightly")],
    ..Options::default()
  })
  .expect_err("mode");
  assert!(err.contains("usage: cargo xtask bench"), "{err}");
}
