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
  let rejected =
    |bad: &serde_json::Value| write(&roots, repo, None, bad).expect_err("invalid review recorded");
  for (key, value, error) in [
    (
      "coverage",
      json!([]),
      "coverage does not match changed files",
    ),
    ("findings", json!(["unresolved"]), "findings must be empty"),
    ("checks", json!([]), "checks must be nonempty strings"),
    (
      "worker",
      json!({"id":"worker","host":"claude","model":"model","effort":"high"}),
      "worker host differs from route",
    ),
  ] {
    let mut bad = valid.clone();
    bad[key] = value;
    assert_eq!(rejected(&bad), error, "{key}");
  }
  let mut bad = valid.clone();
  bad["route"]["host"] = json!("hermes");
  bad["worker"]["host"] = json!("hermes");
  assert_eq!(rejected(&bad), "invalid host");
  let mut bad = valid.clone();
  bad["coverage"][1]["docs"] = json!(["missing.md"]);
  assert_eq!(rejected(&bad), "not a committed regular file: missing.md");
  let mut bad = valid.clone();
  bad["coverage"][1]["docs"] = json!(["code.rs"]);
  assert_eq!(rejected(&bad), "not a documentation file: code.rs");
  let mut bad = valid.clone();
  bad["coverage"][1]["disposition"] = json!("updated");
  bad["coverage"][1]["docs"] = json!(["GUIDE.md"]);
  assert_eq!(rejected(&bad), "updated documentation must be in the diff");
  let mut accurate = valid.clone();
  accurate["coverage"][1]["docs"] = json!(["GUIDE.md", "README.md"]);
  write(&roots, repo, None, &accurate).expect("unchanged docs may back an accurate row");
  assert_eq!(
    rejected(&json!({"diff_sha":valid["diff_sha"]})),
    "review version is stale or missing"
  );
}
