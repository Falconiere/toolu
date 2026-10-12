//! The merge-queue scenario rig (#448): the engine scenario with no merge
//! script, workers that commit and push real branches from their real
//! worktrees, and GitHub's PR facts derived from the local origin's git
//! state. "GitHub merges" a PR by squashing its branch into the origin's
//! `main`.

use serde_json::{Value, json};
use toolu_http_test_support::Reply;

use crate::scenario_rig::{Scenario, key};
use crate::server::{Engine, Stop};
use crate::support::git;

/// How a PR's checks stand.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Checks {
  /// Every check passed.
  Green,
  /// A check still runs.
  Running,
}

/// A queue scenario: children launched into worktrees, merges in process.
pub(crate) struct QueueRig {
  pub scenario: Scenario,
  pub engine: Engine,
}

impl QueueRig {
  /// Children `numbers` with no dependencies, all launched.
  pub(crate) fn new(numbers: &[u64]) -> Self {
    let children: Vec<(u64, &[u64])> = numbers.iter().map(|number| (*number, &[][..])).collect();
    let scenario = Scenario::new(&children, u64::try_from(numbers.len()).unwrap_or(3));
    scenario.pool(&["claude", "codex", "opencode"]);
    let mut engine = scenario.engine_with(None);
    assert_eq!(engine.pump().expect("launch"), Stop::Idle);
    let rig = Self { scenario, engine };
    for number in numbers {
      assert_eq!(rig.record(*number)["stage"], "running", "{number}");
    }
    rig
  }

  /// Child `number`'s issue record.
  pub(crate) fn record(&self, number: u64) -> Value {
    self.scenario.record(number)
  }

  fn text(&self, number: u64, field: &str) -> String {
    self.record(number)[field]
      .as_str()
      .unwrap_or_default()
      .to_owned()
  }

  /// The worker's worktree.
  pub(crate) fn worktree(&self, number: u64) -> std::path::PathBuf {
    self.text(number, "worktree").into()
  }

  fn origin(&self) -> std::path::PathBuf {
    self.scenario.tmp.path().join("github.com/o/r.git")
  }

  /// The worker of `number` commits `text` to `file` and pushes its branch.
  pub(crate) fn work(&self, number: u64, file: &str, text: &str) {
    let tree = self.worktree(number);
    git(&tree, &["config", "user.name", "worker"]);
    git(&tree, &["config", "user.email", "worker@localhost"]);
    std::fs::write(tree.join(file), text).expect("write");
    git(&tree, &["add", file]);
    git(&tree, &["commit", "-qm", &format!("work {number}")]);
    let branch = self.text(number, "branch");
    git(&tree, &["push", "-q", "-u", "origin", &branch]);
  }

  /// The PR head on the origin.
  pub(crate) fn head(&self, number: u64) -> String {
    git(&self.origin(), &["rev-parse", &self.text(number, "branch")])
  }

  /// Commits of `main` the PR head lacks.
  fn behind(&self, number: u64) -> u64 {
    let range = format!("{}..main", self.head(number));
    git(&self.origin(), &["rev-list", "--count", &range])
      .parse()
      .unwrap_or(0)
  }

  /// Serve PR `100 + number` as GitHub sees it now.
  pub(crate) fn serve(&self, number: u64, checks: Checks, armed: bool) {
    let (head, behind) = (self.head(number), self.behind(number));
    let rollup = match checks {
      Checks::Green => {
        json!([{"__typename": "CheckRun", "name": "ci", "status": "COMPLETED", "conclusion": "SUCCESS"}])
      }
      Checks::Running => json!([{"__typename": "CheckRun", "name": "ci", "status": "IN_PROGRESS"}]),
    };
    let node = json!({
      "id": format!("PR_{}", 100 + number), "state": "OPEN", "isDraft": false,
      "mergeable": "MERGEABLE", "mergeStateStatus": "CLEAN", "headRefOid": head,
      "headRefName": self.text(number, "branch"), "baseRefName": "main",
      "url": format!("https://github.com/o/r/pull/{}", 100 + number),
      "autoMergeRequest": if armed { json!({"enabledAt": "2026-10-12T00:00:00Z"}) } else { Value::Null },
      "reviewThreads": {"nodes": []},
      "commits": {"nodes": [{"commit": {"statusCheckRollup": {"contexts": {"nodes": rollup}}}}]},
    });
    let github = &self.scenario.github;
    let ok = |body: Value| vec![Reply::new(200, body.to_string())];
    let needle = format!("\"n\":{}", 100 + number);
    let pull = json!({"data": {"repository": {"pullRequest": node}}});
    github
      .sequence_when("/graphql", &needle, ok(pull))
      .expect("graphql");
    let compare = format!("/repos/o/r/compare/main...{head}");
    github
      .sequence(&compare, ok(json!({"behind_by": behind})))
      .expect("compare");
    let workflows = json!({"total_count": 1});
    github
      .sequence("/repos/o/r/actions/workflows", ok(workflows))
      .expect("workflows");
    let repo = json!({"default_branch": "main", "allow_squash_merge": true});
    github.sequence("/repos/o/r", ok(repo)).expect("repo");
    let merge = format!("/repos/o/r/pulls/{}/merge", 100 + number);
    github
      .sequence(&merge, ok(json!({"merged": true})))
      .expect("merge");
  }

  /// GitHub's own clone, on the origin's current `main`.
  fn github_main(&self) -> std::path::PathBuf {
    let clone = self.scenario.tmp.path().join("github-merge");
    if !clone.exists() {
      let origin = self.origin().display().to_string();
      git(
        self.scenario.tmp.path(),
        &["clone", "-q", &origin, "github-merge"],
      );
    }
    git(&clone, &["fetch", "-q", "origin"]);
    git(&clone, &["checkout", "-q", "-B", "main", "origin/main"]);
    clone
  }

  /// GitHub squash-merges PR `100 + number` into `main`.
  pub(crate) fn land(&self, number: u64) {
    let clone = self.github_main();
    let branch = format!("origin/{}", self.text(number, "branch"));
    git(&clone, &["merge", "-q", "--squash", &branch]);
    git(
      &clone,
      &[
        "commit",
        "-qm",
        &format!("Issue {number} (#{})", 100 + number),
      ],
    );
    git(&clone, &["push", "-q", "origin", "main"]);
    self.scenario.close(number);
  }

  /// A human pushes straight to `main`.
  pub(crate) fn push_main(&self, file: &str) {
    let clone = self.github_main();
    std::fs::write(clone.join(file), "human\n").expect("write");
    git(&clone, &["add", file]);
    git(&clone, &["commit", "-qm", "human push"]);
    git(&clone, &["push", "-q", "origin", "main"]);
  }

  /// Workers `numbers` report `ready` together; then the engine runs.
  pub(crate) fn ready(&mut self, numbers: &[u64]) {
    for number in numbers {
      let report = crate::model::Report {
        key: key(*number),
        epic: "e".into(),
        state_dir: self.scenario.state.display().to_string(),
        phase: "ready".into(),
        pr: Some(100 + number),
        note: String::new(),
      };
      self.engine.report(&report).expect("report");
    }
    assert_eq!(self.engine.pump().expect("pump"), Stop::Idle);
  }

  /// The engine's check of PR `100 + number` came back.
  pub(crate) fn checked(&mut self, number: u64) {
    crate::logic::arm_merge(&mut self.engine.world, &key(number));
    assert_eq!(self.engine.pump().expect("pump"), Stop::Idle);
  }

  /// `(name, key, note)` of the journal's queue lines.
  pub(crate) fn queue_journal(&self) -> Vec<(String, String, String)> {
    self
      .scenario
      .journal()
      .into_iter()
      .filter(|row| {
        matches!(
          row.name.as_str(),
          "rebase" | "rebase-skipped" | "queue-left"
        ) || row.kind == "prompt"
      })
      .map(|row| (row.name, row.key, row.note))
      .collect()
  }

  /// The PR numbers GitHub was asked to merge, in order.
  pub(crate) fn merge_calls(&self) -> Vec<String> {
    self
      .scenario
      .github
      .requests()
      .expect("requests")
      .into_iter()
      .filter(|request| request.method == "PUT")
      .map(|request| request.path)
      .collect()
  }

  /// Whether GitHub was asked about PR `100 + number`.
  pub(crate) fn gated(&self, number: u64) -> bool {
    let needle = format!("\"n\":{}", 100 + number);
    self
      .scenario
      .github
      .requests()
      .expect("requests")
      .iter()
      .any(|request| String::from_utf8_lossy(&request.body).contains(&needle))
  }
}
