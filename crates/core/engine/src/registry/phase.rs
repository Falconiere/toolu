//! The registry phase of one walk (`runRegistry` in `registry-run.ts`): every
//! entry of the event directory in byte order is gated, checked for shadowing
//! and run, until one stops the walk. Warnings go to stderr as they happen; the
//! results are returned for the walk to fold afterwards, as TypeScript does.

mod batching;

use std::collections::{BTreeMap, BTreeSet};

use toolu_protocol::decision::Decision;
use toolu_protocol::event::HostEvent;
use toolu_runtime::registry::rule::RuleContext;
use toolu_runtime::registry::{ModuleKind, RegistryEvent};

use super::executable::run_executable;
use super::gate::plugin_active;
use super::manifests::{Manifests, Read};
use super::{Entry, list_dir};
use crate::dispatch::event::{Payload, View};
use crate::dispatch::fold::{Folded, stops_walk};
use crate::dispatch::output::parse_document;
use crate::dispatch::session::Session;
use crate::dispatch::walk::encoded;
use crate::dispatch::{ModuleResult, Phase};
use crate::trace::{Skip, Step, StepKind, StepStatus};
use batching::flush;

/// One walk, as the registry phase needs it.
pub(crate) struct RegistryWalk<'w> {
  pub(crate) payload: &'w Payload,
  pub(crate) session: &'w Session<'w>,
  pub(crate) view: &'w View,
  pub(crate) ctx: &'w RuleContext<'w>,
  pub(crate) event: HostEvent,
}

/// The phase's running state: results, stderr and trace, and whether it stopped.
struct Run<'r> {
  walk: &'r RegistryWalk<'r>,
  out: Vec<Folded>,
  stderr: &'r mut String,
  trace: &'r mut Vec<Step>,
  stopped: bool,
}

impl Run<'_> {
  fn step(&mut self, entry: &Entry, kind: StepKind, status: StepStatus) {
    self.trace.push(Step {
      module: entry.file.clone(),
      kind,
      status,
    });
  }

  fn warn(&mut self, line: &str) {
    self.stderr.push_str(line);
    self.stderr.push('\n');
  }

  /// A decision of `entry`, folded later; it stops the walk when it is the phase's stop kind.
  fn decided(&mut self, entry: &Entry, kind: StepKind, decision: &Decision) {
    self.step(entry, kind, StepStatus::Decided);
    let stdout = encoded(self.walk.session.host, self.walk.event, decision);
    self.stopped = matches!(
      (self.walk.session.phase, decision),
      (Phase::Pre, Decision::Deny { .. }) | (Phase::Post, Decision::Block { .. })
    );
    self.out.push(Folded {
      name: entry.file.clone(),
      result: ModuleResult {
        stdout,
        ..ModuleResult::default()
      },
      truncated: false,
    });
  }
}

/// The registry event a phase dispatches from.
fn registry_event(phase: Phase) -> RegistryEvent {
  match phase {
    Phase::Pre => RegistryEvent::ToolPre,
    Phase::Post => RegistryEvent::ToolPost,
  }
}

/// Run the registry for one walk; returns the results to fold, in order.
pub(crate) fn run_registry(
  walk: &RegistryWalk<'_>,
  stderr: &mut String,
  trace: &mut Vec<Step>,
) -> Vec<Folded> {
  let session = walk.session;
  let event = registry_event(session.phase);
  let dir = session.config_root.join("toolu").join(event.dir_name());
  let listing = list_dir(&dir);
  let mut run = Run {
    walk,
    out: Vec::new(),
    stderr,
    trace,
    stopped: false,
  };
  for file in &listing.rejected {
    run.warn(&format!(
      "toolu-registry: registry module {file} lacks <plugin-spec>__<name> namespace; skipped"
    ));
  }
  let manifests = Manifests::read(&listing.entries, event, session.options.rules);
  let mut admit = Admission::new(session, &listing.entries, &manifests);
  let mut batch: Vec<&Entry> = Vec::new();
  for entry in &listing.entries {
    if let Some(skip) = admit.skip(entry) {
      run.step(entry, step_kind(entry.kind), StepStatus::Skipped(skip));
      continue;
    }
    match entry.kind {
      ModuleKind::Esm => batch.push(entry),
      ModuleKind::Bash => {
        flush(&mut run, &mut batch);
        if !run.stopped {
          executable(&mut run, entry);
        }
      }
      ModuleKind::Manifest => manifest(&mut run, &mut batch, &manifests, entry),
    }
    if run.stopped {
      return run.out;
    }
  }
  flush(&mut run, &mut batch);
  run.out
}

/// Gating and shadowing for one directory.
struct Admission<'a> {
  session: &'a Session<'a>,
  /// Specs with a usable manifest: their `.js` and `.sh` modules are shadowed.
  usable: BTreeSet<String>,
  /// Specs with a `.js` module: their `.sh` modules are shadowed.
  esm: BTreeSet<String>,
  /// Whether each spec's plugin is active, asked once per walk.
  active: BTreeMap<String, bool>,
}

impl<'a> Admission<'a> {
  fn new(session: &'a Session<'a>, entries: &[Entry], manifests: &Manifests<'_>) -> Admission<'a> {
    let esm = entries
      .iter()
      .filter(|entry| entry.kind == ModuleKind::Esm)
      .map(|entry| entry.spec.clone())
      .collect();
    Admission {
      session,
      usable: manifests.usable_specs(),
      esm,
      active: BTreeMap::new(),
    }
  }

  /// Why `entry` does not run, if it does not.
  fn skip(&mut self, entry: &Entry) -> Option<Skip> {
    let session = self.session;
    let active = *self.active.entry(entry.spec.clone()).or_insert_with(|| {
      selected(session, &entry.spec) && plugin_active(&entry.spec, &session.roots)
    });
    let shadowed = match entry.kind {
      ModuleKind::Manifest => false,
      ModuleKind::Esm => self.usable.contains(&entry.spec),
      ModuleKind::Bash => self.usable.contains(&entry.spec) || self.esm.contains(&entry.spec),
    };
    if !active {
      Some(Skip::Inactive)
    } else if shadowed {
      Some(Skip::Shadowed)
    } else {
      None
    }
  }
}

fn step_kind(kind: ModuleKind) -> StepKind {
  match kind {
    ModuleKind::Esm => StepKind::Esm,
    ModuleKind::Bash => StepKind::Executable,
    ModuleKind::Manifest => StepKind::Rule,
  }
}

/// Whether the host-resolved selection, if any, includes `spec`.
fn selected(session: &Session<'_>, spec: &str) -> bool {
  session
    .options
    .selected_specs
    .is_none_or(|specs| specs.contains(spec))
}

/// Run one `.sh` entry and decide whether it stops the walk.
fn executable(run: &mut Run<'_>, entry: &Entry) {
  let folded = run_executable(entry, run.walk.payload, run.walk.session);
  let code = folded.result.exit_code;
  run.step(entry, StepKind::Executable, StepStatus::Exited(code));
  let doc = parse_document(&folded.result.stdout).ok();
  run.stopped =
    code == 2 || code == 0 && !folded.truncated && stops_walk(run.walk.session.phase, doc.as_ref());
  run.out.push(folded);
}

/// One manifest: read, matcher, rule, `applies`, then the rule's decision.
fn manifest(run: &mut Run<'_>, batch: &mut Vec<&Entry>, manifests: &Manifests<'_>, entry: &Entry) {
  let (manifest, rule) = match manifests.get(&entry.file) {
    Some(Read::Valid(manifest, rule)) => (manifest, rule),
    Some(Read::Problem(reason)) => return problem(run, batch, entry, reason.clone()),
    None => return,
  };
  let not_matching = StepStatus::Skipped(Skip::NotMatching);
  if !manifest.matches(&run.walk.payload.tool_name) {
    return run.step(entry, StepKind::Rule, not_matching);
  }
  let Some(rule) = rule else {
    let version = env!("CARGO_PKG_VERSION");
    let reason = format!(
      "no rule {}__{} in toolu {version}",
      manifest.spec, manifest.name
    );
    return problem(run, batch, entry, reason);
  };
  if !rule.applies(&run.walk.view.event, run.walk.ctx) {
    return run.step(entry, StepKind::Rule, not_matching);
  }
  flush(run, batch);
  if !run.stopped {
    let decision = rule.run(&run.walk.view.event, run.walk.ctx);
    run.decided(entry, StepKind::Rule, &decision);
  }
}

/// A manifest that cannot enable its rule: one stderr line, never an error.
fn problem(run: &mut Run<'_>, batch: &mut Vec<&Entry>, entry: &Entry, reason: String) {
  flush(run, batch);
  if run.stopped {
    return;
  }
  run.warn(&format!(
    "toolu-registry: manifest {} skipped: {reason}",
    entry.file
  ));
  run.step(entry, StepKind::Rule, StepStatus::Failed(reason));
}

#[cfg(test)]
#[path = "tests/phase_test.rs"]
mod tests;
