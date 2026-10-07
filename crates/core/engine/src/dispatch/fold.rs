//! Folding module results into one walk (`consume` and `settle` in
//! `dispatch-walk.ts`). A module exit of 2 ends the walk with that module's
//! stderr; any other non-zero exit is reported and skipped. Before a tool the
//! first deny ends the walk and the first ask is held; after it the first
//! `decision: "block"` ends the walk. Everything else is advice.

use super::output::{Advisories, final_advisory, final_ask, parse_document, printed, read_field};
use super::{ModuleResult, Phase};

/// What one walk has gathered so far.
#[derive(Debug, Clone)]
pub(crate) struct WalkState {
  pub(crate) phase: Phase,
  pub(crate) advisories: Advisories,
  pub(crate) ask: Option<String>,
  pub(crate) stderr: String,
}

/// One module's result as the fold sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Folded {
  /// The name stderr lines use: the built-in's name or the registry file.
  pub(crate) name: String,
  /// Its stdout, stderr and exit status.
  pub(crate) result: ModuleResult,
  /// Its output ran past the budget and was cut.
  pub(crate) truncated: bool,
}

/// The host event name merged output carries.
pub(crate) fn event_name(phase: Phase) -> &'static str {
  match phase {
    Phase::Pre => "PreToolUse",
    Phase::Post => "PostToolUse",
  }
}

impl WalkState {
  /// An empty walk.
  pub(crate) fn new(phase: Phase) -> WalkState {
    WalkState {
      phase,
      advisories: Advisories::default(),
      ask: None,
      stderr: String::new(),
    }
  }

  /// Folds one result in; the hook's final result when it ends the walk.
  pub(crate) fn consume(&mut self, folded: &Folded) -> Option<ModuleResult> {
    let Folded {
      name,
      result,
      truncated,
    } = folded;
    if result.exit_code == 2 {
      return Some(ModuleResult {
        stdout: String::new(),
        stderr: format!("{}{}", self.stderr, result.stderr),
        exit_code: 2,
      });
    }
    if *truncated {
      let limit = super::MAX_OUTPUT_BYTES;
      let line =
        format!("toolu-dispatch: module {name} printed more than {limit} bytes; output skipped\n");
      self.stderr.push_str(&line);
      return None;
    }
    if result.exit_code != 0 {
      let line = format!(
        "toolu-dispatch: module {name} exited {}; output skipped\n",
        result.exit_code
      );
      self.stderr.push_str(&line);
      return None;
    }
    if result.stdout.is_empty() {
      return None;
    }
    let doc = parse_document(&result.stdout).ok();
    if stops_walk(self.phase, doc.as_ref()) {
      return Some(ModuleResult {
        stdout: printed(&result.stdout),
        stderr: self.stderr.clone(),
        exit_code: 0,
      });
    }
    if self.phase == Phase::Pre && permission_of(doc.as_ref()) == "ask" {
      self.ask.get_or_insert_with(|| result.stdout.clone());
      return None;
    }
    self.advisories.collect(doc.as_ref());
    None
  }

  /// The walk's result once every module ran: the held ask, else the advisories.
  pub(crate) fn settle(self) -> ModuleResult {
    let stdout = match &self.ask {
      Some(ask) => final_ask(ask, &self.advisories),
      None => final_advisory(&self.advisories, event_name(self.phase)),
    };
    ModuleResult {
      stdout,
      stderr: self.stderr,
      exit_code: 0,
    }
  }
}

/// `.hookSpecificOutput.permissionDecision // empty`.
fn permission_of(doc: Option<&toolu_runtime::json::ordered::Ordered>) -> String {
  read_field(doc, &["hookSpecificOutput", "permissionDecision"])
}

/// Whether a result ends the walk: a deny before the tool, a block after it.
pub(crate) fn stops_walk(
  phase: Phase,
  doc: Option<&toolu_runtime::json::ordered::Ordered>,
) -> bool {
  match phase {
    Phase::Pre => permission_of(doc) == "deny",
    Phase::Post => read_field(doc, &["decision"]) == "block",
  }
}

#[cfg(test)]
#[path = "tests/fold_test.rs"]
mod tests;
