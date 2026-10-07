//! The shared ledger golden (#421, AC-1, AC-2): `toolu ledger` replays every
//! run of `fixtures/ledger/cases.json`, the file the Bun bundles were captured
//! from, and reproduces its streams, exit code and branch ledger once the
//! sandbox paths and the parts that change from run to run are normalized the
//! way `plugins/toolu/hooks/src/__tests__/ledger-cases.ts` normalizes them.

#[path = "helpers/ledger.rs"]
mod ledger;

use std::collections::BTreeMap;
use std::path::Path;

use ledger::{Project, Res};
use regex::Regex;
use serde::{Deserialize, Serialize};

const FIXTURE: &str = concat!(
  env!("CARGO_MANIFEST_DIR"),
  "/../../fixtures/ledger/cases.json"
);

/// `fixtures/ledger/cases.json`.
#[derive(Deserialize)]
struct Fixture {
  cases: Vec<Case>,
}

/// One case: steps over one sandbox, with variables for every run.
#[derive(Deserialize)]
struct Case {
  name: String,
  #[serde(default)]
  env: BTreeMap<String, String>,
  steps: Vec<Step>,
}

/// A file to write, a script to run, or a CLI run to compare.
#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "lowercase")]
enum Step {
  Write { path: String, body: String },
  Sh { script: String },
  Run(Run),
}

/// `plan-ledger` or `verdict` with its argv, and what it printed.
#[derive(Deserialize)]
struct Run {
  cli: String,
  argv: Vec<String>,
  cwd: Option<String>,
  #[serde(default)]
  env: BTreeMap<String, String>,
  expect: Observed,
}

/// Streams, exit code and the branch ledger, normalized; a missing ledger is null.
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct Observed {
  exit_code: i32,
  stdout: String,
  stderr: String,
  #[serde(default)]
  ledger: Option<String>,
}

/// The parts of an output that change from run to run, and their placeholders.
fn volatile() -> Res<Vec<(Regex, &'static str)>> {
  Ok(vec![
    (
      Regex::new(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d(\.\d{3})?Z")?,
      "<TS>",
    ),
    (Regex::new(r"\(\d+s\)")?, "(<N>s)"),
    (Regex::new(r"\b[0-9a-f]{40}\b")?, "<SHA>"),
    (Regex::new(r"\(diff [0-9a-f]{12}\)")?, "(diff <SHA>)"),
  ])
}

/// One case's sandbox and the rules that normalize what it prints.
struct Replay {
  project: Project,
  volatile: Vec<(Regex, &'static str)>,
}

impl Replay {
  /// `$PROJECT` and `$HOME` in a case string.
  fn expand(&self, text: &str) -> String {
    let project = self.project.root.display().to_string();
    let home = self.project.home.display().to_string();
    text.replace("$PROJECT", &project).replace("$HOME", &home)
  }

  /// Output as the fixture records it.
  fn normalize(&self, text: &str) -> String {
    let project = self.project.root.display().to_string();
    let home = self.project.home.display().to_string();
    let mut text = text.replace(&project, "$PROJECT").replace(&home, "$HOME");
    for (pattern, placeholder) in &self.volatile {
      text = pattern.replace_all(&text, *placeholder).into_owned();
    }
    text
  }

  /// One run, observed and normalized like the fixture's `expect`.
  fn observe(&self, case: &Case, run: &Run) -> Res<Observed> {
    let mut argv = match run.cli.as_str() {
      "plan-ledger" => vec!["ledger".to_owned()],
      "verdict" => vec!["ledger".to_owned(), "verdict".to_owned()],
      other => return Err(format!("unknown cli {other}").into()),
    };
    argv.extend(run.argv.iter().map(|word| self.expand(word)));
    let cwd = run
      .cwd
      .as_deref()
      .map_or_else(|| self.project.root.clone(), |dir| self.expand(dir).into());
    let env = case
      .env
      .iter()
      .chain(&run.env)
      .map(|(key, value)| (key, self.expand(value)));
    let output = self
      .project
      .command(Path::new(&cwd))
      .args(&argv)
      .envs(env)
      .output()?;
    Ok(Observed {
      exit_code: output.status.code().unwrap_or(-1),
      stdout: self.normalize(&String::from_utf8_lossy(&output.stdout)),
      stderr: self.normalize(&String::from_utf8_lossy(&output.stderr)),
      ledger: std::fs::read_to_string(self.project.ledger())
        .ok()
        .map(|text| self.normalize(&text)),
    })
  }

  /// One run against its record; a difference is the error.
  fn check(&self, case: &Case, run: &Run, index: usize) -> Res<()> {
    let observed = self.observe(case, run)?;
    if observed == run.expect {
      return Ok(());
    }
    let (expected, observed) = (json(&run.expect)?, json(&observed)?);
    Err(format!("run {index}: expected {expected}\nobserved {observed}").into())
  }

  /// Every step of `case`; the first run that differs from its record is the error.
  fn run(&self, case: &Case) -> Res<()> {
    let mut index = 0;
    for step in &case.steps {
      match step {
        Step::Write { path, body } => self.project.write(&self.expand(path), &self.expand(body))?,
        Step::Sh { script } => {
          self.project.sh(&self.expand(script))?;
        }
        Step::Run(run) => {
          self.check(case, run, index)?;
          index += 1;
        }
      }
    }
    Ok(())
  }
}

fn json(observed: &Observed) -> Res<String> {
  Ok(serde_json::to_string_pretty(observed)?)
}

/// `case` in its own sandbox.
fn replay(case: &Case) -> Result<(), String> {
  let replay = Replay {
    project: Project::new().map_err(|err| err.to_string())?,
    volatile: volatile().map_err(|err| err.to_string())?,
  };
  replay.run(case).map_err(|err| err.to_string())
}

#[test]
fn every_case_of_the_shared_golden_replays_byte_for_byte() {
  let text = std::fs::read_to_string(FIXTURE).unwrap();
  let cases = serde_json::from_str::<Fixture>(&text).unwrap().cases;
  assert!(cases.len() >= 27, "the golden lost cases: {}", cases.len());
  let failures: Vec<String> = std::thread::scope(|scope| {
    let handles: Vec<_> = cases
      .iter()
      .map(|case| scope.spawn(move || replay(case)))
      .collect();
    cases
      .iter()
      .zip(handles)
      .filter_map(|(case, handle)| match handle.join() {
        Ok(Ok(())) => None,
        Ok(Err(err)) => Some(format!("{}: {err}", case.name)),
        Err(_) => Some(format!("{}: panicked", case.name)),
      })
      .collect()
  });
  assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}
