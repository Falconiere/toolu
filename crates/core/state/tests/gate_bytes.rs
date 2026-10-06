//! The shared gate-file byte golden (AC-3): `fixtures/state/gate-bytes.json`
//! was captured once from the TypeScript writers; every step's file bytes,
//! clear outcome and drop log come out the same from Rust.
//! (`packages/toolu-core/src/state/__tests__/gate-bytes.test.ts` holds
//! TypeScript to the same golden.)

use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{ClearOutcome, GateFailure, clear_gate_file, record_gate_failure};
use toolu_state::time::parse_iso;

type Res<T> = Result<T, String>;

fn cases() -> Res<Vec<Value>> {
  let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/state/gate-bytes.json");
  let text = std::fs::read_to_string(&file).map_err(|err| err.to_string())?;
  let doc: Value = serde_json::from_str(&text).map_err(|err| err.to_string())?;
  let cases = doc
    .get("cases")
    .and_then(Value::as_array)
    .ok_or("no cases")?;
  Ok(cases.clone())
}

fn text<'a>(value: &'a Value, key: &str) -> Res<&'a str> {
  value
    .get(key)
    .and_then(Value::as_str)
    .ok_or_else(|| format!("no {key} in {value}"))
}

/// The file at `path`, or `None` when absent: the golden's `null`.
fn contents(path: &Path) -> Option<String> {
  std::fs::read_to_string(path).ok()
}

fn expected(step: &Value, key: &str) -> Option<String> {
  step.get("expect")?.get(key)?.as_str().map(str::to_owned)
}

/// Runs one case in `dir`; every mismatch is returned, not panicked.
fn run_case(case: &Value, dir: &Path) -> Res<Vec<String>> {
  let project = dir.join("project");
  let gate: PathBuf = project.join(".claude/tmp/quality-gate-status.json");
  std::fs::create_dir_all(project.join(".claude/tmp")).map_err(|err| err.to_string())?;
  if let Some(initial) = case.get("initial").and_then(Value::as_str) {
    std::fs::write(&gate, initial).map_err(|err| err.to_string())?;
  }
  let env = Env::from_pairs([
    ("HOME", dir.join("home").display().to_string()),
    ("TOOLU_PROJECT_DIR", project.display().to_string()),
  ]);
  let mut ctx = StateCtx::new(Roots::new(env, Some(Host::Claude)));
  let steps = case
    .get("steps")
    .and_then(Value::as_array)
    .ok_or("no steps")?;
  let mut wrong = Vec::new();
  for (at, step) in steps.iter().enumerate() {
    ctx.now = Some(parse_iso(text(step, "now")?).ok_or("bad now")?);
    let (file, source) = (text(step, "file")?, text(step, "source")?);
    if text(step, "op")? == "record" {
      let (reason, violations) = (text(step, "reason")?, text(step, "violations")?);
      record_gate_failure(
        &mut ctx,
        &gate,
        &GateFailure {
          file,
          source,
          reason,
          violations,
        },
      );
    } else {
      let outcome = match clear_gate_file(&mut ctx, &gate, file, source) {
        ClearOutcome::Cleared => "cleared",
        ClearOutcome::Noop => "noop",
      };
      if expected(step, "outcome").as_deref() != Some(outcome) {
        wrong.push(format!("step {at}: outcome {outcome}"));
      }
    }
    if contents(&gate) != expected(step, "bytes") {
      wrong.push(format!("step {at}: bytes {:?}", contents(&gate)));
    }
    let log = PathBuf::from(format!("{}.dropped.log", gate.display()));
    if contents(&log) != expected(step, "dropLog") {
      wrong.push(format!("step {at}: drop log {:?}", contents(&log)));
    }
  }
  Ok(wrong)
}

#[test]
fn every_golden_sequence_is_reproduced_byte_for_byte() {
  let cases = cases().unwrap();
  assert_eq!(cases.len(), 16);
  for case in &cases {
    let dir = tempfile::tempdir().unwrap();
    let wrong = run_case(case, dir.path()).unwrap();
    assert_eq!(wrong, Vec::<String>::new(), "{}", case["name"]);
  }
}
