//! The CI half of the workflow contract: triggers, pinned actions and least
//! privilege, the `tests.yml` aggregates and Rust legs, and the review job.

use std::path::Path;

use regex::Regex;
use serde_json::Value;

use crate::workflow_yaml::{Findings, at, includes, items, lists, runs, step_where, steps, text};

/// The jobs `gate` aggregates.
const GATED: [&str; 6] = ["changes", "docs", "fuzz", "hook-bench", "rust", "rust-musl"];

/// The jobs allowed `contents: write`, as `<file>:<job>`.
const WRITERS: [&str; 3] = [
  "release-please.yml:native",
  "release-native.yml:upload",
  "release-native.yml:publish",
];

/// The parsed workflows by file name, missing ones read as `null`.
pub(crate) struct Docs<'d>(pub(crate) &'d [(String, Value)]);

impl Docs<'_> {
  /// The workflow `name`, or `null`.
  pub(crate) fn get(&self, name: &str) -> &Value {
    self
      .0
      .iter()
      .find(|(file, _)| file == name)
      .map_or(&Value::Null, |(_, doc)| doc)
  }
}

/// Whether `event` of `doc` targets `main`.
fn on_main(doc: &Value, event: &str) -> bool {
  lists(at(doc, &["on", event, "branches"]), "main")
}

/// Whether `event` of `doc` has no path filter.
fn unfiltered(doc: &Value, event: &str) -> bool {
  let filters = ["paths", "paths-ignore"];
  filters
    .iter()
    .all(|key| at(doc, &["on", event, key]).is_null())
}

/// Whether `doc` declares the trigger `event`.
fn has(doc: &Value, event: &str) -> bool {
  at(doc, &["on"]).get(event).is_some()
}

/// Every workflow keeps its triggers and never filters a required one by path.
pub(crate) fn triggers(docs: &Docs<'_>, found: &mut Findings) {
  let tests = docs.get("tests.yml");
  found.need(
    on_main(tests, "push")
      && on_main(tests, "pull_request")
      && has(tests, "workflow_dispatch")
      && unfiltered(tests, "push")
      && unfiltered(tests, "pull_request"),
    "tests.yml must run on main push/PR and manual dispatch without path filters",
  );
  let review = docs.get("toolu-review.yml");
  found.need(
    lists(at(review, &["on", "pull_request", "types"]), "synchronize")
      && unfiltered(review, "pull_request"),
    "toolu-review.yml must review synchronized PRs without path filters",
  );
  let (please, native) = (
    docs.get("release-please.yml"),
    docs.get("release-native.yml"),
  );
  found.need(
    on_main(please, "push")
      && has(please, "workflow_dispatch")
      && has(native, "workflow_call")
      && has(native, "workflow_dispatch")
      && has(docs.get("release-finalize.yml"), "workflow_call"),
    "release workflows must retain push, reusable, and dry-run triggers",
  );
}

/// Least-privilege `contents` and a pinned version on every third-party action.
pub(crate) fn actions_and_permissions(name: &str, doc: &Value, found: &mut Findings) {
  found.need(
    text(at(doc, &["permissions", "contents"])) == "read",
    &format!("{name} must declare least-privilege default contents permission"),
  );
  let pinned = Regex::new(r"@(?:v\d+(?:\.\d+)*|[a-f0-9]{40})$").ok();
  let jobs = at(doc, &["jobs"]).as_object().into_iter().flatten();
  let holders = std::iter::once(("*", doc)).chain(jobs.map(|(id, job)| (id.as_str(), job)));
  for (id, holder) in holders {
    let writes = text(at(holder, &["permissions", "contents"])) == "write";
    found.need(
      !writes || WRITERS.contains(&format!("{name}:{id}").as_str()),
      &format!("{name}:{id} has unnecessary contents: write"),
    );
    if id == "*" {
      continue;
    }
    for action in std::iter::once(holder).chain(steps(holder)) {
      let used = text(at(action, &["uses"]));
      if used.is_empty() || used.starts_with("./") {
        continue;
      }
      found.need(
        pinned.as_ref().is_some_and(|expr| expr.is_match(used)),
        &format!("{name}:{id} action {used} is not version-pinned"),
      );
    }
  }
}

/// `gate` aggregates every gated job with `always()`.
pub(crate) fn aggregates(tests: &Value, found: &mut Findings) {
  let job = at(tests, &["jobs", "gate"]);
  let mut needs: Vec<&str> = items(at(job, &["needs"])).iter().map(text).collect();
  needs.sort_unstable();
  found.need(
    needs == GATED && includes(at(job, &["if"]), "always()"),
    "tests.yml:gate must aggregate every gated job with always()",
  );
  found.need(
    text(at(job, &["name"])) == "gate",
    "tests.yml:gate must report the gate check",
  );
  found.need(
    runs(job).contains("cargo xtask ci-aggregate tests.yml"),
    "tests.yml:gate must run cargo xtask ci-aggregate",
  );
  found.need(
    at(tests, &["jobs", "ts"]).is_null(),
    "tests.yml must not keep the ts job",
  );
  found.need(
    runs(at(tests, &["jobs", "changes"])).contains("cargo xtask ci-changes"),
    "tests.yml:changes must run cargo xtask ci-changes",
  );
}

/// The Linux leg owns the full gate, the macOS leg runs clippy and tests.
pub(crate) fn rust_legs(tests: &Value, found: &mut Findings) {
  let rust = at(tests, &["jobs", "rust"]);
  found.need(
    includes(at(rust, &["name"]), "matrix.check"),
    "tests.yml:rust must report each OS check name",
  );
  let legs = items(at(rust, &["strategy", "matrix", "include"]));
  for (os, check) in [("ubuntu-24.04", "rust"), ("macos-14", "rust-macos")] {
    found.need(
      legs
        .iter()
        .any(|leg| text(at(leg, &["os"])) == os && text(at(leg, &["check"])) == check),
      &format!("tests.yml:rust lacks {check} on {os}"),
    );
  }
  leg_steps(rust, found);
  let command = runs(rust);
  for required in [
    "cargo xtask gate --only guardrails",
    "cargo xtask gate --only layers",
    "cargo xtask gate",
    "@ast-grep/cli",
    "python3 -B -m unittest discover -s .github/scripts",
  ] {
    found.need(
      command.contains(required),
      &format!("tests.yml:rust lacks {required}"),
    );
  }
  musl(tests, found);
}

/// The Linux leg's gate steps and the macOS leg's clippy and tests.
fn leg_steps(rust: &Value, found: &mut Findings) {
  let linux = [
    "Reject Rust suppressions and structural violations",
    "Check Rust crate layers",
    "cargo xtask gate",
  ];
  let named = |name: &str, os: &str| {
    step_where(rust, |step| {
      text(at(step, &["name"])) == name && text(at(step, &["if"])) == os
    })
  };
  found.need(
    linux
      .iter()
      .all(|name| !named(name, "matrix.os == 'ubuntu-24.04'").is_null()),
    "tests.yml:rust Linux leg must own the full quality gate",
  );
  let mac = named("macOS clippy and Rust tests", "matrix.os == 'macos-14'");
  found.need(
    includes(
      at(mac, &["run"]),
      "cargo xtask gate --only clippy --only tests",
    ),
    "tests.yml:rust macOS leg must run clippy and Rust tests",
  );
}

fn musl(tests: &Value, found: &mut Findings) {
  let job = at(tests, &["jobs", "rust-musl"]);
  found.need(
    includes(at(job, &["if"]), "needs.changes.outputs.rust"),
    "tests.yml:rust-musl must follow rust paths",
  );
  found.need(
    runs(job).contains("musl-tools"),
    "tests.yml:rust-musl needs the C toolchain",
  );
  let legs = items(at(job, &["strategy", "matrix", "include"]));
  for target in ["x86_64-unknown-linux-musl", "aarch64-unknown-linux-musl"] {
    found.need(
      legs.iter().any(|leg| text(at(leg, &["target"])) == target),
      &format!("tests.yml:rust-musl lacks {target}"),
    );
  }
}

/// The review job exempts only the same-repo release PR and gets the Rust context.
pub(crate) fn review(root: &Path, doc: &Value, found: &mut Findings) {
  let job = at(doc, &["jobs", "review"]);
  let branch = "release-please--branches--main--components--toolu";
  let same_repo = "github.event.pull_request.head.repo.full_name != github.repository";
  found.need(
    includes(
      at(job, &["if"]),
      &format!("github.head_ref != '{branch}' || {same_repo}"),
    ),
    "toolu-review.yml:review must exempt only the same-repo release PR",
  );
  let action = step_where(job, |step| includes(at(step, &["uses"]), "code-review@v8"));
  let with = |key: &str| at(action, &["with", key]);
  found.need(
    !text(with("CODEBASE_OVERVIEW")).trim().is_empty() && text(with("RULES_MAX_BYTES")) == "65536",
    "toolu-review.yml must supply crate context and the full rules budget",
  );
  found.need(
    root.join(".github/code-review-prompt.md").is_file(),
    "toolu-review.yml Rust prompt file is missing",
  );
  for (key, value) in [
    ("REVIEW_PROMPT_FILE", ".github/code-review-prompt.md"),
    ("EXCLUDE_GLOBS", "fixtures/**"),
    ("EXCLUDE_GLOBS", "docs/cli/**"),
  ] {
    found.need(
      includes(with(key), value),
      &format!("toolu-review.yml:review lacks {key} {value}"),
    );
  }
}

#[cfg(test)]
#[path = "tests/workflow_ci_test.rs"]
mod tests;
