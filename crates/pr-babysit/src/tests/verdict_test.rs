use std::collections::BTreeSet;

use serde_json::Value;

use super::parse;
use crate::fixture;

const RECORDED: [&str; 7] = [
  "pr31-verdict.txt",
  "pr120-verdict-changes.txt",
  "pr122-verdict-approved.txt",
  "pr157-must-fix.txt",
  "provider-error.txt",
  "in-progress.txt",
  "no-checkbox.txt",
];

#[test]
fn captured_comments_reproduce_the_recorded_golden() {
  let golden = fixture::json("parse-verdict.golden.json");
  for name in RECORDED {
    let input = fixture::text(&format!("verdicts/{name}"));
    assert_eq!(parse(&input), golden["recorded"][name], "{name}");
  }
}

#[test]
fn pr_31_keeps_six_distinct_low_severity_keys() {
  let result = parse(&fixture::text("verdicts/pr31-verdict.txt"));
  assert_eq!(result["state"], "complete");
  assert_eq!(result["verdict"], "approved");
  let findings = result["findings"].as_array().unwrap();
  assert_eq!(findings.len(), 6);
  let keys: BTreeSet<&str> = findings.iter().filter_map(|f| f["key"].as_str()).collect();
  assert_eq!(keys.len(), 6);
  assert_eq!(findings[0]["path"], "plugins/toolu/hooks/session-start.sh");
  assert_eq!(findings[0]["line"], 17);
  assert_eq!(findings[0]["severity"], "low");
  assert_eq!(findings[2]["line"], Value::Null);
}

#[test]
fn a_provider_failure_is_incomplete_despite_a_changes_label() {
  let result = parse(&fixture::text("verdicts/provider-error.txt"));
  assert_eq!(result["state"], "provider_error");
  assert_eq!(result["complete"], false);
  assert_eq!(result["verdict"], "changes");
  assert_eq!(result["findings"], serde_json::json!([]));
  assert_eq!(result["must_fix"], serde_json::json!([]));
}

#[test]
fn boundary_inputs_reproduce_the_recorded_golden() {
  let golden = fixture::json("parse-verdict.golden.json");
  let cases = [
    ("empty", String::new()),
    ("unrelated", "fyi see actions/runs/999 and agent-merge-foo, thanks".to_owned()),
    (
      "checklist label wins over finding prose",
      [
        "### Code Review — x",
        "- [x] Set verdict label (`merge-approved`)",
        "### Findings",
        "`a/b.sh:9`: low: the label should be `request-changes` here.",
        "`request-changes`",
      ]
      .join("\n"),
    ),
    ("standalone chip identifies a review", "some prose\n- [x] done\n`request-changes`".to_owned()),
    (
      "prose verdict fallback",
      "### Code Review — x\n- [x] done\n**Verdict:** ⚠️ Changes requested\n### Findings (0)".to_owned(),
    ),
    (
      "finding quotes provider error",
      "### Code Review — x\n**Verdict:** ⚠️ Changes requested\n### Findings (1)\n`a.sh:1`: medium: **Provider error:** is shown on failure\n`request-changes`".to_owned(),
    ),
    (
      "Top-N bare and marked sentences",
      "### Code Review — x\n### Top-N must-fix (4)\nFix bare line.\n- Fix dash item.\n* Fix star item.\n1. Fix numbered item.\n<details>\n`request-changes`".to_owned(),
    ),
    (
      "approved with Top-N",
      "### Code Review — x\n**Verdict:** ✅ Approved\n### Findings (0)\n### Top-N must-fix\nSomething the bot still wants.\n<details>\n`merge-approved`".to_owned(),
    ),
  ];
  for (name, input) in cases {
    assert!(!golden["boundary"][name].is_null(), "{name}");
    assert_eq!(parse(&input), golden["boundary"][name], "{name}");
  }
}

#[test]
fn every_verdict_pattern_compiles() {
  for pattern in [
    &super::REVIEW_MARKER,
    &super::UNCHECKED,
    &super::CHECKED,
    &super::LABEL_ITEM,
    &super::LABEL_CHIP,
    &super::LABEL_ANY,
    &super::VERDICT_LINE,
    &super::CHANGES_REQUESTED,
    &super::APPROVED,
    &super::CHANGES_BOLD,
    &super::APPROVED_BOLD,
    &super::PROVIDER,
    &super::FINDINGS_HEAD,
    &super::ANY_HEAD,
    &super::FINDING,
    &super::PATH_LINE,
    &super::TOP_N_HEAD,
    &super::TOP_N_END,
    &super::BULLET,
    &super::NUMBERED,
  ] {
    assert!(crate::pattern::compiles(pattern));
  }
}
