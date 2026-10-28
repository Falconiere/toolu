//! Review workflow routing must retain findings and fail closed.

#[test]
fn native_review_routes_failures_without_losing_findings_or_fabricating_state() {
  let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let body =
    std::fs::read_to_string(repo.join("plugins/code-review/skills/review/SKILL.md")).unwrap();
  for required in [
    "automatically invoke",
    "toolu-review:review",
    "not-enabled",
    "unconfigured",
    "outages",
    "deadlines",
    "state errors",
    "non-JSON failures",
    "incomplete coverage",
    "Retain and assess every real finding",
    "never route fallback back",
    "without writing approval",
    "path-scoped review",
    "supplied base",
    "clean worktree",
    "complete native review with blocking findings requires fixes",
  ] {
    assert!(
      body.contains(required),
      "native review contract missing: {required}"
    );
  }
  for path in [
    "plugins/toolu/workflows/review-and-commit.md",
    "plugins/pr-babysit/workflows/babysit.md",
    "plugins/delivery-flow/skills/delivery-flow/references/execution.md",
  ] {
    let body = std::fs::read_to_string(repo.join(path)).unwrap();
    for required in [
      "code-review:review",
      "toolu-review:review",
      "unavailable",
      "without approval",
    ] {
      assert!(body.contains(required), "{path}: missing {required}");
    }
  }
}
