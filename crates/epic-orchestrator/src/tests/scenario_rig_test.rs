//! An engine scenario: a GitHub epic `o/r#1` served by the HTTPS fixture,
//! its children's checkout behind a local origin under `github.com/o/r.git`
//! (so the graph resolves it), the herdr socket fixture with real worktrees
//! and agent processes, the merge script seam, and an engine on the inline
//! executor. The test plays the workers by sending their reports.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{Value, json};
use toolu_http_test_support::{Fixture, Reply};
use toolu_runtime::env::Env;

use crate::effects_epic::{EpicJob, run};
use crate::herdr_fixture::HerdrFixture;
use crate::journal::{self, Record};
use crate::model::{Action, Report};
use crate::paths::Paths;
use crate::server::{Engine, Fault};
use crate::support::FAST;

/// One child of the epic: its tracker state and the children blocking it.
struct Child {
  state: String,
  blocked_by: Vec<u64>,
  body: String,
}

/// Everything one scenario touches.
pub(crate) struct Scenario {
  pub tmp: tempfile::TempDir,
  pub herdr: HerdrFixture,
  pub github: Fixture,
  pub env: Env,
  pub paths: Paths,
  pub state: PathBuf,
  pub scripts: PathBuf,
  children: Mutex<BTreeMap<u64, Child>>,
}

/// An agent that records a Claude and a Codex session for its worktree, then
/// stays alive; each host captures its own.
const AGENT: &str = r#"slug=$(printf %s "$PWD" | tr -c 'A-Za-z0-9' '-'); dir="{claude}/projects/$slug"; mkdir -p "$dir"; printf '{"cwd":"%s","sessionId":"0199a272-0b4e-7cc3-8c4f-071c81275655"}\n' "$PWD" > "$dir/s.jsonl"; day="{codex}/sessions/$(date -u +%Y/%m/%d)"; mkdir -p "$day"; printf '{"type":"session_meta","payload":{"cwd":"%s","id":"0199a272-0b4e-7cc3-8c4f-071c81275656"}}\n' "$PWD" > "$day/rollout-$$.jsonl"; exec sleep 600"#;

impl Scenario {
  /// Children `(number, blocked_by)` of `o/r#1`, at most `max` live at once.
  pub(crate) fn new(children: &[(u64, &[u64])], max: u64) -> Self {
    Self::with_bodies(children, max, &[])
  }

  /// As [`Scenario::new`], with issue bodies `(number, body)`.
  pub(crate) fn with_bodies(children: &[(u64, &[u64])], max: u64, bodies: &[(u64, &str)]) -> Self {
    let scenario = Self::base(children);
    for (number, body) in bodies {
      if let Some(child) = scenario.children.lock().expect("children").get_mut(number) {
        (*body).clone_into(&mut child.body);
      }
    }
    scenario.publish("open");
    scenario.graph(("o/r#1", "github", ""), max);
    scenario
  }

  /// The checkout, herdr, the merge script and `opencode`; no tracker routes.
  pub(crate) fn base(children: &[(u64, &[u64])]) -> Self {
    let tmp = tempfile::tempdir().expect("temp");
    let herdr = HerdrFixture::start();
    let github = Fixture::start().expect("fixture");
    let root = tmp.path();
    let path = |rel: &str| root.join(rel).display().to_string();
    let env = herdr
      .env()
      .with(
        "PATH",
        &format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", path("bin")),
      )
      .with("HOME", &path("home"))
      .with("TOOLU_RESOURCE_HOME", &path("resources"))
      .with("CLAUDE_CONFIG_DIR", &path("claude"))
      .with("CODEX_HOME", &path("codex"))
      .with("EPIC_STATE_HOME", &path("epics"));
    herdr.state().agent_script = AGENT
      .replace("{claude}", &path("claude"))
      .replace("{codex}", &path("codex"));
    let rows = children
      .iter()
      .map(|(number, blocked)| {
        let child = Child {
          state: "open".into(),
          blocked_by: blocked.to_vec(),
          body: String::new(),
        };
        (*number, child)
      })
      .collect();
    let scenario = Self {
      paths: Paths::at(root.join("resources")),
      state: root.join("state"),
      scripts: root.join("scripts"),
      children: Mutex::new(rows),
      tmp,
      herdr,
      github,
      env,
    };
    scenario.checkout();
    scenario.merge_script();
    scenario.opencode();
    scenario
  }

  /// `src/r`, cloned from `github.com/o/r.git`, registered with herdr.
  fn checkout(&self) {
    let root = self.tmp.path();
    let origin = root.join("github.com/o/r.git");
    let checkout = root.join("src/r");
    crate::support::origin_with_clone(&origin, &checkout, &[("a.txt", "one\n")]);
    self.herdr.add_repo(&checkout);
  }

  fn merge_script(&self) {
    executable(
      &self.scripts.join("merge"),
      "cat >/dev/null\necho '{\"outcome\":\"applied\"}'",
    );
    executable(
      &self.scripts.join("reconcile"),
      "cat >/dev/null\necho '{\"state\":\"merged\"}'",
    );
  }

  /// A 1.x `opencode` that lists one session for the worktree it runs in.
  fn opencode(&self) {
    let body = r#"case "$1" in
  --version) echo 1.4.0 ;;
  session) printf '[{"id":"ses_0123456789abcdef","directory":"%s","created":%s}]' "$PWD" "$(($(date +%s) * 1000 + 999))" ;;
esac"#;
    executable(&self.tmp.path().join("bin/opencode"), body);
  }
}

/// The tracker the fixture serves.
impl Scenario {
  /// Serve the epic, its children and their pull requests as they stand.
  pub(crate) fn publish(&self, epic_state: &str) {
    for (path, body) in self.routes(epic_state) {
      let reply = Reply::new(200, body.to_string());
      self.github.route(&path, reply).expect("route");
    }
  }

  fn routes(&self, epic_state: &str) -> Vec<(String, Value)> {
    let children = self.children.lock().expect("children");
    let item = |number: u64| {
      let (state, body) = children.get(&number).map_or(("open", ""), |child| {
        (child.state.as_str(), child.body.as_str())
      });
      json!({"repository_url": self.github.url("/repos/o/r"), "number": number,
        "title": format!("Issue {number}"), "state": state, "body": body,
        "html_url": format!("https://github.com/o/r/issues/{number}")})
    };
    let subs: Vec<Value> = children.keys().map(|number| item(*number)).collect();
    let epic = json!({"title": "Epic", "state": epic_state, "html_url": "u", "body": ""});
    let mut routes = vec![
      ("/repos/o/r/issues/1".to_owned(), epic),
      (
        "/repos/o/r/issues/1/sub_issues?per_page=100".to_owned(),
        json!(subs),
      ),
      ("/repos/o/r/issues/1/comments".to_owned(), json!({})),
      ("/repos/o/r".to_owned(), json!({"default_branch": "main"})),
      (
        "/graphql".to_owned(),
        json!({"data": {"repository": {"issue": null}}}),
      ),
    ];
    for (number, child) in children.iter() {
      let blockers: Vec<Value> = child.blocked_by.iter().map(|b| item(*b)).collect();
      let base = format!("/repos/o/r/issues/{number}");
      routes.push((format!("{base}/dependencies/blocked_by"), json!(blockers)));
      routes.push((format!("{base}/comments"), json!({})));
      routes.push((base, item(*number)));
      let pull = json!({"merged": true, "state": "closed", "number": 100 + number});
      routes.push((format!("/repos/o/r/pulls/{}", 100 + number), pull));
    }
    routes
  }

  /// Mark child `number` closed in the tracker, as its PR's merge does.
  pub(crate) fn close(&self, number: u64) {
    if let Some(child) = self.children.lock().expect("children").get_mut(&number) {
      "closed".clone_into(&mut child.state);
    }
    self.publish("open");
  }

  /// Build `graph.json` from the tracker `(reference, kind, default repo)`
  /// and register the epic as `e`.
  pub(crate) fn graph(&self, tracker: (&str, &str, &str), max: u64) {
    let built = run(Action::Refresh, &self.epic_job(tracker, max));
    assert_eq!(built["outcome"], "applied", "{built}");
    std::fs::create_dir_all(&self.paths.root).expect("engine root");
    let registry = json!({"version": 1, "epics": [{"key": "e", "state_dir": self.state}]});
    std::fs::write(self.paths.registry(), registry.to_string()).expect("registry");
  }

  fn epic_job(&self, (reference, kind, repo): (&str, &str, &str), max: u64) -> EpicJob {
    EpicJob {
      state_dir: self.state.clone(),
      reference: reference.into(),
      tracker: Some(kind.into()),
      default_repo: repo.into(),
      max_parallel: max,
      recovered: false,
      env: self.env.clone(),
      paths: self.paths.clone(),
      reach: crate::support::reach(&self.github, FAST),
    }
  }

  /// The engine over this scenario, effects inline, merging by script.
  pub(crate) fn engine(&self) -> Engine {
    self.engine_with(Some(self.scripts.clone()))
  }

  /// The engine with `scripts` as its merge seam; `None` defers merges.
  pub(crate) fn engine_with(&self, scripts: Option<PathBuf>) -> Engine {
    let mut engine = Engine::open(self.paths.clone(), scripts, Fault::None).expect("open");
    engine.env = self.env.clone();
    engine.reach = crate::support::reach(&self.github, FAST);
    engine
  }

  /// The worker of child `number` reports `phase` with its PR.
  pub(crate) fn report(&self, engine: &mut Engine, number: u64, phase: &str) {
    let report = Report {
      key: key(number),
      epic: "e".into(),
      state_dir: self.state.display().to_string(),
      phase: phase.to_owned(),
      pr: Some(100 + number),
      note: String::new(),
    };
    engine.report(&report).expect("report");
    let _stop = engine.pump().expect("pump");
  }

  /// The host pool: `kinds` with one slot each.
  pub(crate) fn pool(&self, kinds: &[&str]) {
    let rows: Vec<Value> = kinds
      .iter()
      .map(|kind| json!({"kind": kind, "cap": 1}))
      .collect();
    std::fs::write(self.state.join("pool.json"), json!(rows).to_string()).expect("pool");
  }

  /// Child `number`'s agent process dies, as a lost pane's does.
  pub(crate) fn lose_agent(&self, number: u64) {
    let mut state = self.herdr.state();
    if let Some(index) = state
      .agents
      .iter()
      .position(|agent| agent.name == key(number))
    {
      let mut agent = state.agents.remove(index);
      crate::herdr_state::stop(&mut agent.process);
    }
  }

  /// The worker of child `number` reports `phase` with `note`.
  pub(crate) fn report_note(&self, engine: &mut Engine, number: u64, phase: &str, note: &str) {
    let report = Report {
      key: key(number),
      epic: "e".into(),
      state_dir: self.state.display().to_string(),
      phase: phase.to_owned(),
      pr: None,
      note: note.to_owned(),
    };
    engine.report(&report).expect("report");
    let _stop = engine.pump().expect("pump");
  }

  /// Every journal line so far.
  pub(crate) fn journal(&self) -> Vec<Record> {
    journal::tail(&self.paths.journal_dir(), std::time::SystemTime::now()).expect("journal")
  }

  /// The record of child `number`.
  pub(crate) fn record(&self, number: u64) -> Value {
    let path = self
      .state
      .join("issues")
      .join(format!("{}.json", key(number)));
    crate::disk::read_value(&path).expect("record")
  }
}

/// The issue key of child `number`.
pub(crate) fn key(number: u64) -> String {
  format!("o-r-{number}")
}

/// Write an executable `sh` script at `path`.
fn executable(path: &std::path::Path, body: &str) {
  std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
  std::fs::write(path, format!("#!/bin/sh\n{body}\n")).expect("script");
  let mut perms = std::fs::metadata(path).expect("meta").permissions();
  std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
  std::fs::set_permissions(path, perms).expect("chmod");
}
