//! Black-box contract for the #670 eval harness, offline: the committed golden set
//! loads, every case materializes into a real Git repository the production `shape`
//! and `validate` stages accept, the matcher classifies findings grounded in those
//! diffs, the gate contract holds against the production predicates, and seeded
//! review memory behaves as specified. No network, no model.

#[path = "helpers/replay.rs"]
mod replay;

use std::path::{Path, PathBuf};

use toolu_code_review::contract::{Finding, Severity, Side};
use toolu_code_review::eval::{Case, Fixtures, Gate, Status, match_findings, violations};
use toolu_code_review::memory::{self, Memory};
use toolu_code_review::model::ModelFinding;
use toolu_code_review::shape::{Shaped, shape};
use toolu_code_review::validate::{Input, Validated, validate};
use toolu_runtime::env::Env;

fn root() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/code-review-eval")
}

fn golden() -> Result<Fixtures, String> {
  Fixtures::load(&root())
}

fn need<T>(value: Option<T>, what: &str) -> Result<T, String> {
  value.ok_or_else(|| format!("missing {what}"))
}

fn case<'a>(fixtures: &'a Fixtures, id: &str) -> Result<&'a Case, String> {
  need(fixtures.set().case(id), id)
}

fn git_env() -> Env {
  Env::process()
    .with("GIT_CONFIG_NOSYSTEM", "1")
    .with("GIT_CONFIG_GLOBAL", "/dev/null")
    .with("XDG_CONFIG_HOME", "/dev/null")
}

/// The case's committed diff, shaped by the production stage.
fn diff_of(fixtures: &Fixtures, case: &Case) -> Result<Shaped, String> {
  let repo = fixtures.materialize_repo(case)?;
  shape(&git_env(), repo.path(), Some(fixtures.base_rev(case)))
    .map_err(|error| format!("{}: {error:?}", case.id))
}

/// A model finding citing the first changed new-side line containing `needle`.
fn cite(shaped: &Shaped, path: &str, needle: &str, claim: &str) -> Result<ModelFinding, String> {
  let file = need(shaped.files.iter().find(|file| file.path == path), path)?;
  let line = need(
    file
      .lines
      .iter()
      .find(|line| line.side == Side::New && line.text.contains(needle)),
    needle,
  )?;
  Ok(ModelFinding {
    path: path.to_owned(),
    line: line.line,
    end_line: line.line,
    side: Side::New,
    severity: Severity::High,
    category: "correctness".to_owned(),
    claim: claim.to_owned(),
    quote: line.text.clone(),
    suggestion: None,
    prior_id: None,
  })
}

fn grounded(shaped: &Shaped, memory: &Memory, model: &[ModelFinding]) -> Validated {
  validate(&Input {
    shaped,
    model,
    deterministic: &[],
    memory,
    redactions: &[],
    unreviewed: &[],
  })
}

fn statuses(case: &Case, findings: &[Finding]) -> Vec<Status> {
  match_findings(&case.expectations, findings)
    .matches
    .iter()
    .map(|entry| entry.status)
    .collect()
}

fn none() -> Vec<String> {
  Vec::new()
}

#[test]
fn every_case_materializes_and_shapes_a_nonempty_committed_diff() {
  let fixtures = golden().expect("fixture");
  assert_eq!(fixtures.set().cases.len(), 7);
  for case in &fixtures.set().cases {
    let shaped = diff_of(&fixtures, case).expect("fixture");
    assert!(!shaped.files.is_empty(), "{} has no diff", case.id);
    assert_eq!(shaped.coverage.unreviewed.len(), 0, "{}", case.id);
    assert!(!shaped.uncommitted, "{}", case.id);
  }
}

#[test]
fn the_probe_defects_are_caught_and_the_known_false_positive_is_flagged() {
  let fixtures = golden().expect("fixture");
  let underflow = case(&fixtures, "slice-underflow").expect("fixture");
  let shaped = diff_of(&fixtures, underflow).expect("fixture");
  let model = [
    cite(
      &shaped,
      "src/lib.rs",
      "items.len() - count",
      "The slice underflows when count exceeds the length",
    )
    .expect("fixture"),
    cite(
      &shaped,
      "src/lib.rs",
      "items.len() - count",
      "Prefer rewrite with split_at for clarity",
    )
    .expect("fixture"),
  ];
  let out = grounded(&shaped, &Memory::default(), &model);
  assert_eq!(out.findings.len(), 2);
  assert_eq!(
    statuses(underflow, &out.findings),
    [Status::Caught, Status::Flagged]
  );

  let ownership = case(&fixtures, "inverted-ownership").expect("fixture");
  let shaped = diff_of(&fixtures, ownership).expect("fixture");
  let found = cite(
    &shaped,
    "src/access.rs",
    "owner != caller",
    "The ownership check is inverted, so non-owners may delete",
  )
  .expect("fixture");
  let out = grounded(&shaped, &Memory::default(), &[found]);
  assert_eq!(statuses(ownership, &out.findings), [Status::Caught]);
}

#[test]
fn the_cross_file_defect_is_caught_in_the_file_that_divides() {
  let fixtures = golden().expect("fixture");
  let cross = case(&fixtures, "cross-file-defect").expect("fixture");
  let shaped = diff_of(&fixtures, cross).expect("fixture");
  let paths: Vec<&str> = shaped.files.iter().map(|file| file.path.as_str()).collect();
  assert_eq!(paths, ["src/limits.rs", "src/pages.rs"]);
  let found = cite(
    &shaped,
    "src/pages.rs",
    "total / page_size()",
    "limits now returns a page size of zero, so this division panics",
  )
  .expect("fixture");
  let out = grounded(&shaped, &Memory::default(), &[found]);
  assert_eq!(
    statuses(cross, &out.findings),
    [Status::Caught, Status::Clear]
  );
}

#[test]
fn a_finding_no_row_claims_is_left_unclassified_and_a_silent_review_misses() {
  let fixtures = golden().expect("fixture");
  let underflow = case(&fixtures, "slice-underflow").expect("fixture");
  let shaped = diff_of(&fixtures, underflow).expect("fixture");
  let stray = cite(
    &shaped,
    "src/lib.rs",
    "items.len() - count",
    "The subtraction should use a named constant",
  )
  .expect("fixture");
  let out = grounded(&shaped, &Memory::default(), &[stray]);
  let outcome = match_findings(&underflow.expectations, &out.findings);
  assert_eq!(outcome.unclassified.len(), 1);
  assert!(!outcome.defects_satisfied());
  assert_eq!(outcome.tally().missed, 1);

  let quiet = match_findings(&underflow.expectations, &[]);
  assert_eq!(quiet.tally().missed, 1);
  assert_eq!(quiet.tally().clear, 1);
}

#[test]
fn the_prompt_false_positive_is_flagged_only_when_raised() {
  let fixtures = golden().expect("fixture");
  let prompt = case(&fixtures, "prompt-false-positive").expect("fixture");
  let shaped = diff_of(&fixtures, prompt).expect("fixture");
  let raised = cite(
    &shaped,
    "src/hint.rs",
    "password",
    "A hardcoded credential is committed in the prompt text",
  )
  .expect("fixture");
  let out = grounded(&shaped, &Memory::default(), &[raised]);
  assert_eq!(statuses(prompt, &out.findings), [Status::Flagged]);
  assert_eq!(statuses(prompt, &[]), [Status::Clear]);
}

#[test]
fn every_gate_flag_in_the_golden_set_holds_against_the_production_predicates() {
  let fixtures = golden().expect("fixture");
  let gates: Vec<(&str, Gate)> = fixtures
    .set()
    .cases
    .iter()
    .filter_map(|case| case.gate.map(|gate| (case.id.as_str(), gate)))
    .collect();
  assert!(gates.len() >= 3);
  for (id, gate) in &gates {
    assert_eq!(violations(gate), none(), "{id}");
  }
  assert!(
    gates
      .iter()
      .any(|(_, gate)| gate.unreviewed_never_writes_pass)
  );
}

fn seeded(fixtures: &Fixtures, id: &str) -> Result<(Shaped, Memory), String> {
  let case = case(fixtures, id)?;
  let memory = need(fixtures.memory(case)?, "memory.json")?;
  Ok((diff_of(fixtures, case)?, memory))
}

#[test]
fn a_drifted_line_stays_open_under_its_prior_id() {
  let fixtures = golden().expect("fixture");
  let (shaped, memory) = seeded(&fixtures, "memory-drift").expect("fixture");
  let found = cite(
    &shaped,
    "src/lib.rs",
    "items.len() - count",
    "slice underflows when count exceeds length",
  )
  .expect("fixture");
  // The defect sat on line 2 of the base and sits on line 6 now.
  assert_eq!(found.line, 6);
  let out = grounded(&shaped, &memory, &[found]);
  assert_eq!(out.recap.still_open, ["mem-drift-1"]);
  assert_eq!(out.recap.resolved, none());
  assert_eq!(out.recap.new, none());
  assert_eq!(
    out.findings.first().map(|finding| finding.id.as_str()),
    Some("mem-drift-1")
  );
  let drift = case(&fixtures, "memory-drift").expect("fixture");
  assert_eq!(statuses(drift, &out.findings), [Status::Caught]);

  // Re-examined and not re-raised, the same prior would be resolved instead.
  let silent = grounded(&shaped, &memory, &[]);
  assert_eq!(silent.recap.resolved, ["mem-drift-1"]);
  assert_eq!(silent.recap.still_open, none());
}

#[test]
fn an_unreexamined_prior_stays_open_and_is_never_resolved() {
  let fixtures = golden().expect("fixture");
  let (shaped, memory) = seeded(&fixtures, "memory-unreexamined").expect("fixture");
  assert!(shaped.files.iter().all(|file| file.path != "src/legacy.rs"));
  let out = grounded(&shaped, &memory, &[]);
  assert_eq!(out.recap.still_open, ["mem-legacy-1"]);
  assert_eq!(out.recap.resolved, none());
}

#[test]
fn missing_history_means_a_full_review_with_nothing_carried_over() {
  let fixtures = golden().expect("fixture");
  let dir = tempfile::tempdir().expect("fs");
  let state = memory::load(&dir.path().join("code-review/feature.json")).expect("fixture");
  assert_eq!(state.memory, Memory::default());
  assert!(state.diff_sha.is_empty() && state.passes.is_empty() && !state.complete);

  let (shaped, seed) = seeded(&fixtures, "memory-missing").expect("fixture");
  assert_eq!(seed, state.memory);
  assert_eq!(shaped.coverage.reviewed, shaped.gate_files);
  let found = cite(
    &shaped,
    "src/lib.rs",
    "value * 3",
    "double no longer doubles: it now triples",
  )
  .expect("fixture");
  let out = grounded(&shaped, &state.memory, &[found]);
  assert_eq!(out.recap.new.len(), 1);
  assert_eq!(out.recap.still_open, none());
  assert_eq!(out.recap.resolved, none());
  let missing = case(&fixtures, "memory-missing").expect("fixture");
  assert_eq!(statuses(missing, &out.findings), [Status::Caught]);
}

#[test]
fn the_fixture_directory_is_committed_without_git_metadata() {
  let mut stack = vec![root()];
  while let Some(dir) = stack.pop() {
    for entry in std::fs::read_dir(&dir).expect("fs") {
      let entry = entry.expect("fs");
      assert_ne!(entry.file_name(), ".git", "{}", dir.display());
      if entry.file_type().expect("fs").is_dir() {
        stack.push(entry.path());
      }
    }
  }
}
