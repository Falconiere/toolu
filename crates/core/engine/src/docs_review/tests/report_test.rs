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
  accurate["documentation_coverage"][1] = json!({"path":"GUIDE.md","disposition":"accurate","note":"Checked guide","sources":["code.rs"]});
  write(&roots, repo, None, &accurate).expect("unchanged docs may back an accurate row");
  assert_eq!(
    rejected(&json!({"diff_sha":valid["diff_sha"]})),
    "review version is stale or missing"
  );
}

#[test]
fn malformed_documentation_evidence_is_rejected_when_written_and_assessed() {
  let (dir, roots) = fixture();
  let repo = dir.path();
  let valid = report(&roots, repo);
  let path = write(&roots, repo, None, &valid).expect("valid review");
  let mut cases = Vec::new();
  let mut bad = valid.clone();
  bad["documentation_coverage"].as_array_mut().unwrap().pop();
  cases.push(("missing row", bad));
  let mut bad = valid.clone();
  bad["documentation_coverage"]
    .as_array_mut()
    .unwrap()
    .push(valid["documentation_coverage"][0].clone());
  cases.push(("duplicate row", bad));
  let mut bad = valid.clone();
  bad["documentation_coverage"][1]["path"] = json!("missing.md");
  cases.push(("unknown document", bad));
  for (name, sources) in [
    ("unknown source", json!(["missing.rs"])),
    ("duplicate source", json!(["code.rs", "code.rs"])),
    ("missing source", json!([])),
  ] {
    let mut bad = valid.clone();
    bad["documentation_coverage"][0]["sources"] = sources;
    cases.push((name, bad));
  }
  let mut bad = valid.clone();
  bad["documentation_coverage"][1] =
    json!({"path":"GUIDE.md","disposition":"updated","note":"Changed guide","sources":["code.rs"]});
  cases.push(("unchanged updated document", bad));
  let mut bad = valid;
  bad["documentation_coverage"][0]["disposition"] = json!("not-needed");
  cases.push(("cited document not needed", bad));
  for (name, bad) in cases {
    assert!(
      write(&roots, repo, None, &bad).is_err(),
      "write accepted {name}"
    );
    std::fs::write(&path, bad.to_string()).expect("corrupt receipt");
    assert!(
      crate::docs_review::assess(&roots, repo, "feature").is_err(),
      "assessment accepted {name}"
    );
  }
}

fn excluded_citation_fixture() -> (tempfile::TempDir, toolu_runtime::host::roots::Roots) {
  let (dir, roots) = fixture();
  let repo = dir.path();
  super::super::tests::git(repo, &["checkout", "main"]);
  std::fs::create_dir_all(repo.join("docs/toolu/plans")).unwrap();
  std::fs::write(
    repo.join("docs/toolu/plans/history.md"),
    "historical context\n",
  )
  .unwrap();
  std::fs::create_dir_all(repo.join("private")).unwrap();
  std::fs::write(repo.join("private/notes.md"), "private context\n").unwrap();
  std::fs::create_dir_all(repo.join(".claude")).unwrap();
  std::fs::write(
    repo.join(".claude/toolu.config.json"),
    r#"{"docsSync":{"surfaceExcludes":["private/*"]}}"#,
  )
  .unwrap();
  super::super::tests::git(repo, &["add", "."]);
  super::super::tests::git(repo, &["commit", "-m", "add supplemental documentation"]);
  super::super::tests::git(repo, &["checkout", "feature"]);
  super::super::tests::git(repo, &["merge", "--no-edit", "main"]);
  (dir, roots)
}

#[test]
fn excluded_documents_can_supplement_affected_citations() {
  let (dir, roots) = excluded_citation_fixture();
  let repo = dir.path();
  let mut review = report(&roots, repo);
  for excluded in ["docs/toolu/plans/history.md", "private/notes.md"] {
    assert!(review["documentation_files"].get(excluded).is_none());
  }
  review["coverage"][0]["docs"] = json!(["README.md", "docs/toolu/plans/history.md"]);
  review["coverage"][1]["docs"] = json!(["README.md", "private/notes.md"]);
  write(&roots, repo, None, &review).expect("supplemental citations accepted");
  crate::docs_review::assess(&roots, repo, "feature").expect("supplemental citations assessed");
}

#[test]
fn excluded_citations_alone_and_cited_not_needed_docs_remain_invalid() {
  let (dir, roots) = excluded_citation_fixture();
  let repo = dir.path();
  let valid = report(&roots, repo);
  for excluded in ["docs/toolu/plans/history.md", "private/notes.md"] {
    let mut review = valid.clone();
    review["coverage"][1]["docs"] = json!([excluded]);
    let error = write(&roots, repo, None, &review).expect_err("excluded citation alone");
    assert!(error.contains("affected"), "{error}");
  }
  let mut review = valid;
  review["coverage"][1]["docs"] = json!(["README.md", "GUIDE.md", "private/notes.md"]);
  let error = write(&roots, repo, None, &review).expect_err("cited guide marked not needed");
  assert!(error.contains("GUIDE.md"), "{error}");
}

#[test]
fn updated_coverage_requires_a_changed_affected_document() {
  let (dir, roots) = excluded_citation_fixture();
  let repo = dir.path();
  let excluded = "docs/toolu/plans/history.md";
  std::fs::write(repo.join(excluded), "changed historical context\n").unwrap();
  super::super::tests::git(repo, &["add", excluded]);
  super::super::tests::git(repo, &["commit", "-m", "change historical context"]);
  let mut review = report(&roots, repo);
  review["coverage"].as_array_mut().unwrap().push(json!({
    "path": excluded, "disposition": "not-needed", "note": "Historical context", "docs": []
  }));
  review["coverage"][1]["disposition"] = json!("updated");
  review["coverage"][1]["docs"] = json!(["GUIDE.md", excluded]);
  review["documentation_coverage"][1] = json!({
    "path": "GUIDE.md", "disposition": "accurate", "note": "Checked guide", "sources": ["code.rs"]
  });
  let error = write(&roots, repo, None, &review)
    .expect_err("updated row needs changed affected documentation");
  assert!(
    error.contains("updated") && error.contains("documentation"),
    "{error}"
  );
}
