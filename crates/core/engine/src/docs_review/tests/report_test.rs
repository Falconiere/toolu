use crate::docs_review::{
  tests::{fixture, report},
  write,
};
use serde_json::json;

#[test]
fn invalid_evidence_cannot_be_recorded() {
  let (dir, roots) = fixture();
  let repo = dir.path();
  let valid = report(&roots, repo);
  for (key, value) in [
    ("coverage", json!([])),
    ("findings", json!(["unresolved"])),
    ("checks", json!([])),
    (
      "worker",
      json!({"id":"worker","host":"claude","model":"model","effort":"high"}),
    ),
  ] {
    let mut bad = valid.clone();
    bad[key] = value;
    assert!(write(&roots, repo, None, &bad).is_err(), "{key}");
  }
  let mut bad = valid.clone();
  bad["coverage"][0]["docs"] = json!(["missing.md"]);
  assert!(write(&roots, repo, None, &bad).is_err());
  let mut bad = valid.clone();
  bad["coverage"][1]["disposition"] = json!("updated");
  bad["coverage"][1]["docs"] = json!(["code.rs"]);
  assert!(write(&roots, repo, None, &bad).is_err());
  assert!(write(&roots, repo, None, &json!({"diff_sha":valid["diff_sha"]})).is_err());
}
