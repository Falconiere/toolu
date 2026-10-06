//! The multi-slot quality-gate file (`gate-file.ts`), written byte for byte as
//! TypeScript writes it and under the same `<gate>.lock`, so both share it.
//! Every failing file owns a slot under `entries`; the top level mirrors the
//! latest failure and joins every open violation. A document that parses but
//! fails the strict v1 schema is "unrecognized": recording replaces it (with a
//! warning and a `<gate>.dropped.log` line, so the new failure always lands),
//! clearing leaves it alone.

use std::io::Write as _;
use std::path::Path;

use toolu_runtime::json::ordered::Ordered;

use crate::ctx::StateCtx;
use crate::gate_doc::{
  Failure, cleared_doc, doc_text, dropped_count, failing_doc, seed_entries, single_slot,
};
use crate::gate_schema::{GateEntry, GateFile, validate_gate_file};
use crate::io::write_atomic;
use crate::lock::{LockOptions, with_lock};
use crate::telemetry::{TelemetryEvent, telemetry_append};
use crate::time::iso_seconds;

/// The gate file as read.
#[derive(Debug, Clone, PartialEq)]
pub enum GateRead {
  /// No file.
  Missing,
  /// Unreadable, not JSON, empty, `null` or `false` (jq `-e .` fails).
  Malformed(String),
  /// JSON outside the strict v1 schema: the first issue and the document.
  Unrecognized {
    /// `<path>: <message>`.
    reason: String,
    /// The document as parsed.
    value: Ordered,
  },
  /// A valid document.
  Ok(GateFile),
}

/// Whether a clear changed the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearOutcome {
  /// The entry was dropped and the file rewritten.
  Cleared,
  /// Nothing to clear, or the write failed.
  Noop,
}

/// Reads and classifies the gate file.
pub fn read_gate_file(gate: &Path) -> GateRead {
  if !gate.exists() {
    return GateRead::Missing;
  }
  let text = match std::fs::read_to_string(gate) {
    Ok(text) => text,
    Err(err) => return GateRead::Malformed(err.to_string()),
  };
  let value = match Ordered::parse(&text) {
    Ok(value) => value,
    Err(err) => return GateRead::Malformed(err),
  };
  if value == Ordered::Null || value == Ordered::Bool(false) {
    return GateRead::Malformed(value.to_text(false));
  }
  match validate_gate_file(&value) {
    Ok(doc) => GateRead::Ok(doc),
    Err(reason) => GateRead::Unrecognized { reason, value },
  }
}

/// The root owning `<root>/<host dir>/tmp/quality-gate-status.json`.
fn gate_root(gate: &Path) -> &Path {
  let root = gate.parent().and_then(Path::parent).and_then(Path::parent);
  // `dirname` of a bare relative name is `.`, never the empty path.
  root
    .filter(|root| !root.as_os_str().is_empty())
    .unwrap_or_else(|| Path::new("."))
}

/// A durable line beside the gate file; logging never blocks the write.
fn breadcrumb(gate: &Path, line: &str) {
  let mut log = gate.as_os_str().to_owned();
  log.push(".dropped.log");
  let opened = std::fs::OpenOptions::new()
    .create(true)
    .append(true)
    .open(log);
  let _logged = opened.and_then(|mut file| file.write_all(format!("{line}\n").as_bytes()));
}

/// The failure a gate records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GateFailure<'a> {
  /// The failing file, or [`crate::gate_schema::GLOBAL_GATE_KEY`].
  pub file: &'a str,
  /// The gate recording it.
  pub source: &'a str,
  /// Why it fails.
  pub reason: &'a str,
  /// Its violations, newline-terminated lines.
  pub violations: &'a str,
}

/// `gate_record_failure GATE FILE SOURCE REASON VIOLATIONS`: adds or replaces
/// the failing file's slot.
pub fn record_gate_failure(ctx: &mut StateCtx, gate: &Path, failure: &GateFailure<'_>) {
  let now = iso_seconds(ctx.now());
  let GateFailure {
    file,
    source,
    reason,
    violations,
  } = *failure;
  let f = Failure {
    file,
    source,
    reason,
    violations,
    now: &now,
  };
  let mut lock_warnings = Vec::new();
  let warnings = with_lock(gate, LockOptions::default(), &mut lock_warnings, || {
    record(gate, &f)
  });
  ctx.warnings.extend(lock_warnings);
  ctx.warnings.extend(warnings);
  // One event per recorded failure, re-records included.
  let event = TelemetryEvent::GateFail {
    file: file.to_owned(),
    source: source.to_owned(),
  };
  telemetry_append(ctx, gate_root(gate), &event);
}

/// The locked read-merge-write of a record; its warnings.
fn record(gate: &Path, f: &Failure<'_>) -> Vec<String> {
  let mut warnings = Vec::new();
  let (prev, previous) = match read_gate_file(gate) {
    GateRead::Ok(doc) => (seed_entries(&doc), doc.to_ordered()),
    GateRead::Unrecognized { reason, value } => {
      let dropped = dropped_count(&value, f.file);
      warnings.push(format!(
        "gate-file: unrecognized gate file at {} ({reason}); replacing it",
        gate.display()
      ));
      breadcrumb(
        gate,
        &format!(
          "{} unrecognized gate file replaced; dropped {dropped} entry(ies)",
          f.now
        ),
      );
      (Vec::new(), value)
    }
    GateRead::Missing | GateRead::Malformed(_) => (Vec::new(), Ordered::Object(Vec::new())),
  };
  let body = doc_text(&failing_doc(prev, f).to_ordered());
  if !write_atomic(gate, &body) {
    write_single_slot(gate, &previous, f, &mut warnings);
  }
  warnings
}

/// The fallback when the atomic write fails: a single-slot record, so this
/// failure is never lost.
fn write_single_slot(gate: &Path, previous: &Ordered, f: &Failure<'_>, warnings: &mut Vec<String>) {
  let dropped = dropped_count(previous, f.file);
  if dropped > 0 {
    warnings.push(format!(
      "gate-file: primary write failed at {}; single-slot fallback dropped {dropped} other entry(ies)",
      gate.display()
    ));
    breadcrumb(
      gate,
      &format!(
        "{} primary write failed; single-slot fallback dropped {dropped} entry(ies)",
        f.now
      ),
    );
  }
  // As bash's `> "$gate_file" || true`: nothing further can record it.
  let _written = std::fs::write(gate, doc_text(&single_slot(f)));
}

/// Whether `source` owns `file`'s failure in `doc`.
fn owns(doc: &GateFile, file: &str, source: &str) -> bool {
  match doc {
    GateFile::Passing { .. } => false,
    GateFile::Failing {
      entries: None,
      source: owner,
      file: failed,
      ..
    } => owner == source && failed == file,
    GateFile::Failing {
      entries: Some(entries),
      ..
    } => {
      let slot = entries.iter().find(|(key, _)| key == file);
      slot.map_or("", |(_, entry): &(String, GateEntry)| entry.source.as_str()) == source
    }
  }
}

/// The text a clear would write, or `None` when there is nothing to clear.
fn planned_clear(
  gate: &Path,
  file: &str,
  source: &str,
  now: &str,
  warnings: &mut Vec<String>,
) -> Option<String> {
  let doc = match read_gate_file(gate) {
    GateRead::Ok(doc) => doc,
    GateRead::Malformed(_) => {
      warnings.push(format!(
        "gate-file: malformed JSON at {}; ignoring clear (gate stays failing until next write)",
        gate.display()
      ));
      return None;
    }
    GateRead::Unrecognized { reason, .. } => {
      warnings.push(format!(
        "gate-file: unrecognized gate file at {} ({reason}); ignoring clear",
        gate.display()
      ));
      return None;
    }
    GateRead::Missing => return None,
  };
  if !owns(&doc, file, source) {
    return None;
  }
  let mut left = seed_entries(&doc);
  left.retain(|(key, _)| key != file);
  Some(doc_text(&cleared_doc(left, source, now).to_ordered()))
}

/// `gate_clear_file GATE FILE SOURCE`: drops `file`'s slot if `source` owns
/// it. The common case (no file, passing, another gate's slot) is decided by
/// an unlocked read and takes no lock; a real clear re-plans under the lock.
pub fn clear_gate_file(ctx: &mut StateCtx, gate: &Path, file: &str, source: &str) -> ClearOutcome {
  let now = iso_seconds(ctx.now());
  if planned_clear(gate, file, source, &now, &mut ctx.warnings).is_none() {
    return ClearOutcome::Noop;
  }
  let mut lock_warnings = Vec::new();
  let cleared = with_lock(gate, LockOptions::default(), &mut lock_warnings, || {
    let body = planned_clear(gate, file, source, &now, &mut Vec::new());
    body.is_some_and(|body| write_atomic(gate, &body))
  });
  ctx.warnings.extend(lock_warnings);
  if !cleared {
    return ClearOutcome::Noop;
  }
  let event = TelemetryEvent::GateClear {
    file: file.to_owned(),
    source: source.to_owned(),
  };
  telemetry_append(ctx, gate_root(gate), &event);
  ClearOutcome::Cleared
}

#[cfg(test)]
#[path = "tests/gate_file_test.rs"]
mod tests;
