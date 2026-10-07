//! The `.js` side of the registry phase: pending modules run as one Bun batch,
//! and without Bun a pre-tool module fails closed while a post-tool walk
//! advises once a session.

use toolu_protocol::decision::Decision;
use toolu_protocol::text::Text;

use super::{Run, registry_event};
use crate::dispatch::fold::Folded;
use crate::dispatch::walk::encoded;
use crate::dispatch::{ModuleResult, Phase};
use crate::registry::Entry;
use crate::registry::bridge::bun::claim_advisory;
use crate::registry::bridge::runner::Request;
use crate::registry::bridge::{Batch, Launch, Line, run_batch};
use crate::trace::{Skip, StepKind, StepStatus};

/// The deny a pre-tool `.js` module gets when Bun is missing.
fn missing_bun(file: &str) -> String {
  format!(
    "toolu-registry: module {file} needs Bun 1.4.x, which was not found (checked TOOLU_BUN, PATH and ~/.bun/bin/bun); install it from https://bun.sh or remove the module"
  )
}

/// The post-tool advisory naming the modules that did not run.
fn not_run(files: &[&str]) -> String {
  format!(
    "toolu-registry: {} registry module(s) did not run because Bun was not found: {}. Install Bun 1.4.x from https://bun.sh.",
    files.len(),
    files.join(", ")
  )
}

/// Run the pending `.js` entries through the Bun bridge.
pub(super) fn flush(run: &mut Run<'_>, batch: &mut Vec<&Entry>) {
  if batch.is_empty() || run.stopped {
    batch.clear();
    return;
  }
  let modules = std::mem::take(batch);
  let session = run.walk.session;
  let bun = session.bun.borrow_mut().bun(&session.env);
  let result = match &bun {
    Some(bun) => {
      let launch = Launch {
        bun,
        env: &session.env,
        cwd: session.cwd(),
        module_timeout: session.options.module_timeout,
      };
      let request = Request {
        stop: if session.phase == Phase::Pre {
          "deny"
        } else {
          "post_block"
        },
        registry_event: registry_event(session.phase).slug(),
        event: &run.walk.view.ordered_event,
        ctx: &run.walk.view.ordered_ctx,
      };
      run_batch(&launch, &request, &modules)
    }
    None => Batch {
      no_bun: true,
      ..Batch::default()
    },
  };
  record(run, &modules, result);
}

/// Fold a batch's lines in, then handle the modules Bun never ran.
fn record(run: &mut Run<'_>, modules: &[&Entry], batch: Batch) {
  let reached = batch.lines.len();
  for ((_, line), entry) in batch.lines.into_iter().zip(modules) {
    match line {
      Line::Decision(decision) => run.decided(entry, StepKind::Esm, &decision),
      Line::Error(error) => {
        run.warn(&format!(
          "toolu-registry: module {} failed: {error}; output skipped",
          entry.file
        ));
        run.step(entry, StepKind::Esm, StepStatus::Failed(error));
      }
    }
    if run.stopped {
      break;
    }
  }
  run.stderr.push_str(&batch.stderr);
  if batch.no_bun && !run.stopped {
    no_bun(run, modules.get(reached..).unwrap_or_default());
  }
}

/// No Bun: before a tool the first module fails closed; after it, one advisory a session.
fn no_bun(run: &mut Run<'_>, missing: &[&Entry]) {
  let Some(first) = missing.first() else {
    return;
  };
  if run.walk.session.phase == Phase::Pre {
    let reason = missing_bun(&first.file);
    run.step(first, StepKind::Esm, StepStatus::Skipped(Skip::NoBun));
    if let Ok(reason) = Text::new(reason) {
      push_decision(run, first, &Decision::Deny { reason });
    }
    return;
  }
  for entry in missing {
    run.step(entry, StepKind::Esm, StepStatus::Skipped(Skip::NoBun));
  }
  if !first_advisory(run) {
    return;
  }
  let files: Vec<&str> = missing.iter().map(|entry| entry.file.as_str()).collect();
  let message = not_run(&files);
  if let Ok(message) = Text::new(message) {
    push_decision(run, first, &Decision::Advisory { message });
  }
}

/// Whether this is the session's first missing-Bun advisory: once per call, and
/// once per session through the marker under the project state directory.
fn first_advisory(run: &Run<'_>) -> bool {
  let session = run.walk.session;
  if std::mem::replace(&mut session.bun.borrow_mut().advised, true) {
    return false;
  }
  let raw = &run.walk.view.raw;
  let session_id = raw.get("session_id").and_then(serde_json::Value::as_str);
  let dir = session
    .roots
    .project_state_dir(
      "registry-bridge",
      Some(session.cwd()),
      Some(&session.project_root),
    )
    .ok()
    .flatten();
  claim_advisory(dir.as_deref(), session_id.unwrap_or_default())
}

/// A decision of the dispatcher's own, folded in `entry`'s place without a step.
fn push_decision(run: &mut Run<'_>, entry: &Entry, decision: &Decision) {
  let stdout = encoded(run.walk.session.host, run.walk.event, decision);
  run.stopped = matches!(decision, Decision::Deny { .. }) && run.walk.session.phase == Phase::Pre;
  run.out.push(Folded {
    name: entry.file.clone(),
    result: ModuleResult {
      stdout,
      ..ModuleResult::default()
    },
    truncated: false,
  });
}

#[cfg(test)]
#[path = "tests/batching_test.rs"]
mod tests;
