//! Approval screens a host shows before its first turn (#626). A launch that
//! failed while its agent sits on one has not lost the agent: the screen
//! needs a human, so the failure says which screen and where, and the engine
//! raises `needs-human` instead of an uncertain launch.

use regex::Regex;
use serde_json::Value;

use super::attempt::Attempt;
use super::failure::{APPROVAL, Failure};
use super::lifecycle::Herdr;
use super::record::{self, Record};
use crate::herdr::Call;
use crate::hosts::{HostKind, host_limited};

/// Codex's hook review, as Codex 0.164.0 words it: the startup modal
/// (`18 hooks are new or changed.`) and the hooks browser a prompt's Enter
/// opens from it (`26 hooks need review before they can run.`).
const CODEX_HOOK_REVIEW: &str = r"\b\d+ hooks? (?:is|are) new or changed\.|\b\d+ hooks? needs? review before (?:it|they) can run\.";

/// The approval screens `kind` is known to show.
fn screens(kind: HostKind) -> Option<&'static str> {
  match kind {
    HostKind::Codex => Some(CODEX_HOOK_REVIEW),
    HostKind::Claude | HostKind::Cursor | HostKind::Muse => None,
  }
}

/// The approval screen line `kind` shows in `tail`, if any.
pub(crate) fn approval_screen(kind: HostKind, tail: &str) -> Option<String> {
  let screen = Regex::new(screens(kind)?).ok()?;
  screen.find(tail).map(|found| found.as_str().to_owned())
}

/// The agent a failed attempt left behind.
pub(crate) struct Left<'a> {
  /// Its herdr name.
  pub name: &'a str,
  /// Its host.
  pub kind: HostKind,
  /// Its pane, when the attempt recorded one.
  pub pane: Option<&'a str>,
}

/// `err`, or an `approval_pending` failure naming the screen when the
/// agent's pane shows one. A refusal that started nothing, a provider limit,
/// and a pane that cannot be read keep `err`.
pub(crate) fn explain(err: Failure, herdr: &mut Herdr<'_>, left: &Left<'_>) -> Failure {
  if err.not_started() || host_limited(&err.message) || screens(left.kind).is_none() {
    return err;
  }
  let Ok(read) = herdr.call(&Call::Read {
    target: left.name.to_owned(),
  }) else {
    return err;
  };
  let tail = read
    .pointer("/read/text")
    .and_then(Value::as_str)
    .unwrap_or("");
  if host_limited(tail) {
    return err;
  }
  let Some(line) = approval_screen(left.kind, tail) else {
    return err;
  };
  Failure {
    message: format!(
      "{} is waiting on its hook review in pane {}: \"{line}\"; review and trust the hooks there, then relaunch with --replace",
      left.kind.name(),
      left.pane.unwrap_or("(unrecorded)")
    ),
    exit_code: None,
    code: Some(APPROVAL.to_owned()),
    uncertain: true,
  }
}

impl Attempt<'_> {
  /// [`explain`] for this attempt's agent, in the pane its record names. A
  /// dry run, or an attempt that reserved nothing, started no agent to read.
  pub(super) fn explain(&self, err: Failure, record: &Record, herdr: &mut Herdr<'_>) -> Failure {
    if self.opts.dry_run || self.owner.is_none() {
      return err;
    }
    let left = Left {
      name: &self.issue.key,
      kind: self.host.kind,
      pane: record::text(record, "pane_id"),
    };
    explain(err, herdr, &left)
  }
}

#[cfg(test)]
#[path = "tests/screen_test.rs"]
mod tests;
