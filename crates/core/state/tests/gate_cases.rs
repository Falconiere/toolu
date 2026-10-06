//! The `gate-file` cases of `fixtures/state/cases.json` (AC-4, AC-9):
//! reads, unrecognized replacement and clears, malformed and missing files, a
//! clear behind a live lock, the record's telemetry line and JavaScript key
//! order, one test per scenario, as `gate-file.test.ts` runs them.

#[path = "helpers/cases.rs"]
mod cases;
#[path = "helpers/sandbox.rs"]
mod sandbox;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use cases::{cases_of, field, text};
use sandbox::{Res, Sandbox};
use toolu_protocol::host::Host;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::ordered::Ordered;
use toolu_state::ctx::StateCtx;
use toolu_state::gate_file::{
  ClearOutcome, GateFailure, GateRead, clear_gate_file, read_gate_file, record_gate_failure,
};
use toolu_state::gate_schema::GateFile;
use toolu_state::lock::lock_path;
use toolu_state::time::parse_iso;

/// One case, ready to run: its sandbox, gate path, written body and context.
struct Run {
  case: Ordered,
  sb: Sandbox,
  gate: PathBuf,
  body: Option<String>,
  ctx: StateCtx,
}

impl Run {
  fn record(&mut self) -> Res<()> {
    let (file, source) = (text(&self.case, "file")?, text(&self.case, "source")?);
    let (reason, violations) = (text(&self.case, "reason")?, text(&self.case, "violations")?);
    let failure = GateFailure {
      file: &file,
      source: &source,
      reason: &reason,
      violations: &violations,
    };
    record_gate_failure(&mut self.ctx, &self.gate, &failure);
    Ok(())
  }

  fn clear(&mut self) -> Res<String> {
    let (file, source) = (text(&self.case, "file")?, text(&self.case, "source")?);
    Ok(
      match clear_gate_file(&mut self.ctx, &self.gate, &file, &source) {
        ClearOutcome::Cleared => "cleared".to_owned(),
        ClearOutcome::Noop => "noop".to_owned(),
      },
    )
  }

  fn text(&self, key: &str) -> String {
    text(&self.case, key)
      .unwrap_or_default()
      .replace("$GATE", &self.gate.display().to_string())
  }
}

/// The cases of `scenario`, each in its sandbox (a repository when it names a branch).
fn runs(scenario: &str) -> Res<Vec<Run>> {
  let wanted = Ordered::String(scenario.to_owned());
  let cases = cases_of("state/cases.json", "gate-file")?;
  let mut out = Vec::new();
  for case in cases
    .into_iter()
    .filter(|case| case.get("scenario") == Some(&wanted))
  {
    let sb = Sandbox::new(text(&case, "branch").ok().as_deref())?;
    let gate = sb.project.join(".claude/tmp/quality-gate-status.json");
    std::fs::create_dir_all(sb.project.join(".claude/tmp")).map_err(|err| err.to_string())?;
    let body = case
      .get("document")
      .map(|doc| doc.to_text(true))
      .or_else(|| text(&case, "body").ok());
    if let Some(body) = &body {
      std::fs::write(&gate, body).map_err(|err| err.to_string())?;
    }
    let mut ctx = StateCtx::new(Roots::new(sb.env(), Some(Host::Claude)));
    ctx.now = parse_iso(&text(&case, "now")?);
    out.push(Run {
      case,
      sb,
      gate,
      body,
      ctx,
    });
  }
  Ok(out)
}

fn kind(read: &GateRead) -> &'static str {
  match read {
    GateRead::Missing => "missing",
    GateRead::Malformed(_) => "malformed",
    GateRead::Unrecognized { .. } => "unrecognized",
    GateRead::Ok(_) => "ok",
  }
}

fn read(path: &Path) -> String {
  std::fs::read_to_string(path).unwrap_or_default()
}

fn as_json(value: &Ordered) -> serde_json::Value {
  serde_json::from_str(&value.to_text(false)).unwrap_or_default()
}

#[test]
fn twenty_cases_in_nine_scenarios() {
  assert_eq!(cases_of("state/cases.json", "gate-file").unwrap().len(), 20);
}

#[test]
fn reads_classify_every_document() {
  let all = runs("read").unwrap();
  assert_eq!(all.len(), 10);
  for run in all {
    assert_eq!(
      kind(&read_gate_file(&run.gate)),
      run.text("expectedKind"),
      "{}",
      run.text("name")
    );
  }
  for run in runs("read-reason").unwrap() {
    let GateRead::Unrecognized { reason, .. } = read_gate_file(&run.gate) else {
      panic!("kind")
    };
    assert!(reason.contains(&run.text("reasonContains")), "{reason}");
  }
}

#[test]
fn a_record_replaces_an_unrecognized_document_and_logs_the_drop() {
  for mut run in runs("replace").unwrap() {
    run.record().unwrap();
    let GateRead::Ok(doc) = read_gate_file(&run.gate) else {
      panic!("not ok")
    };
    assert_eq!(
      as_json(&doc.to_ordered()),
      as_json(field(&run.case, "expectedDoc").unwrap())
    );
    let [warning] = &run.ctx.warnings[..] else {
      panic!("{:?}", run.ctx.warnings)
    };
    assert!(warning.starts_with(&run.text("warningPrefix")), "{warning}");
    assert_eq!(
      read(Path::new(&format!("{}.dropped.log", run.gate.display()))),
      run.text("dropLog")
    );
  }
}

#[test]
fn a_clear_leaves_unrecognized_malformed_and_missing_files_alone() {
  for mut run in runs("clear-unrecognized").unwrap() {
    assert_eq!(run.clear().unwrap(), run.text("expected"));
    assert_eq!(Some(read(&run.gate)), run.body);
    let [warning] = &run.ctx.warnings[..] else {
      panic!("{:?}", run.ctx.warnings)
    };
    assert!(warning.ends_with(&run.text("warningSuffix")), "{warning}");
    assert!(!run.sb.project.join(".claude/tmp/telemetry").exists());
  }
  for mut run in runs("clear-malformed").unwrap() {
    assert_eq!(run.clear().unwrap(), run.text("expected"));
    assert_eq!(run.ctx.warnings, [run.text("warning")]);
    assert_eq!(read(&run.gate), run.text("body"));
  }
  for mut run in runs("clear-missing").unwrap() {
    assert_eq!(run.clear().unwrap(), run.text("expected"));
    assert_eq!(run.ctx.warnings, Vec::<String>::new());
  }
}

#[test]
fn a_clear_with_nothing_to_clear_never_waits_on_a_live_lock() {
  for mut run in runs("clear-live-lock").unwrap() {
    let lock = format!("{}{}", std::process::id(), run.text("lockSuffix"));
    std::fs::write(lock_path(&run.gate), &lock).unwrap();
    let started = Instant::now();
    assert_eq!(run.clear().unwrap(), run.text("expected"));
    let Ordered::Number(max) = field(&run.case, "maxMs").unwrap() else {
      panic!("maxMs")
    };
    assert!(started.elapsed() < Duration::from_millis(max.as_u64().unwrap()));
    assert_eq!(run.ctx.warnings, Vec::<String>::new());
    assert_eq!(read(&lock_path(&run.gate)), lock);
  }
}

#[test]
fn a_record_appends_its_telemetry_line_under_the_gate_root() {
  for mut run in runs("telemetry").unwrap() {
    run.record().unwrap();
    let log = run
      .sb
      .project
      .join(".claude/tmp/telemetry")
      .join(run.text("telemetryFile"));
    assert_eq!(read(&log), run.text("expectedLine"));
  }
}

#[test]
fn an_integer_like_key_is_ordered_first() {
  for mut run in runs("ordering").unwrap() {
    run.record().unwrap();
    let GateRead::Ok(GateFile::Failing {
      entries: Some(entries),
      ..
    }) = read_gate_file(&run.gate)
    else {
      panic!("no entries");
    };
    let keys: Vec<Ordered> = entries
      .into_iter()
      .map(|(key, _)| Ordered::String(key))
      .collect();
    assert_eq!(&Ordered::Array(keys), field(&run.case, "expected").unwrap());
  }
}
