//! The hosts the installer targets and how one is chosen: an explicit `--host`
//! always wins, a single host on `PATH` is used without asking, and several are
//! never resolved silently (a terminal asks, otherwise the run refuses). At a
//! terminal, install offers every supported host on the machine, Cursor
//! included when its config directory or `cursor-agent` is there.

use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::args::Scope;
use crate::cursor::store::Store;
use crate::fail::{Fail, Res};
use crate::prompt::Ask;

/// The plugin commands of Claude Code, Codex and Muse, normalized.
pub(crate) mod adapter;
/// The toolu checkout Muse installs from.
pub(crate) mod checkout;
/// Claude Code's plugin listing and update result line.
pub(crate) mod claude;
/// Codex's plugin listing and marketplace commands.
pub(crate) mod codex;
/// Muse's plugin documents and marketplace commands.
pub(crate) mod muse;
/// Host commands with a deadline.
pub(crate) mod run;

/// A host the installer can act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
  /// Claude Code, through `claude plugin`.
  Claude,
  /// Codex, through `codex plugin`.
  Codex,
  /// Muse, through `muse plugins` (#549).
  Muse,
  /// Cursor, through its local plugins directory (#589).
  Cursor,
}

/// The probe order: Muse comes last, so the order Claude Code and Codex users
/// are offered stays as it was.
const PROBED: [Target; 3] = [Target::Claude, Target::Codex, Target::Muse];

impl Target {
  /// The `--host` value, which is also the binary probed on `PATH`.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Target::Claude => "claude",
      Target::Codex => "codex",
      Target::Muse => "muse",
      Target::Cursor => "cursor",
    }
  }

  /// How the host picker names the host.
  fn label(self) -> &'static str {
    match self {
      Target::Claude => "Claude Code",
      Target::Codex => "Codex",
      Target::Muse => "Muse",
      Target::Cursor => "Cursor",
    }
  }

  /// Whether removal on this host reaches only toolu's plugins
  /// (`<name>@toolu`, or Cursor's install marker), so `remove` may hand it the
  /// name of a plugin the marketplace retired (#615). Muse removes by bare
  /// name, from any source. A new target must decide here.
  pub(crate) fn removes_only_toolu(self) -> bool {
    match self {
      Target::Claude | Target::Codex | Target::Cursor => true,
      Target::Muse => false,
    }
  }
}

/// The installer target `--host` names; Hermes is not a target yet. Without a
/// terminal Cursor is never detected (a machine with Cursor and another host
/// would stop resolving silently), so a script names it with `--host cursor`.
pub(crate) fn target_of(host: Host) -> Res<Target> {
  match host {
    Host::Claude => Ok(Target::Claude),
    Host::Codex => Ok(Target::Codex),
    Host::Muse => Ok(Target::Muse),
    Host::Cursor => Ok(Target::Cursor),
    Host::Hermes => Err(Fail::usage(format!(
      "--host {} is not an installer target; toolu plugins installs into claude, codex, muse or cursor",
      host.name()
    ))),
  }
}

/// Whether `bin` is an executable file in a `PATH` directory.
pub(crate) fn on_path(bin: &str, env: &Env) -> bool {
  use std::os::unix::fs::PermissionsExt as _;
  env
    .get("PATH")
    .unwrap_or_default()
    .split(':')
    .filter(|dir| !dir.is_empty())
    .any(|dir| {
      std::fs::metadata(Path::new(dir).join(bin))
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// The hosts whose binary is on `PATH`, in probe order.
pub(crate) fn available(env: &Env) -> Vec<Target> {
  PROBED
    .into_iter()
    .filter(|target| on_path(target.name(), env))
    .collect()
}

/// Every supported host on this machine: those on `PATH`, then Cursor when its
/// config directory exists or `cursor-agent` is on `PATH` (install's picker).
pub(crate) fn detected(env: &Env) -> Vec<Target> {
  let mut found = available(env);
  let cursor = Roots::new(env.clone(), Some(Host::Cursor)).config_root();
  if cursor.is_dir() || on_path("cursor-agent", env) {
    found.push(Target::Cursor);
  }
  found
}

/// The hosts on `PATH`, then Cursor when toolu installed a plugin there, so a
/// leftover `~/.cursor` never asks which host `list`, `remove` or `update` means.
pub(crate) fn with_toolu(env: &Env) -> Vec<Target> {
  let mut found = available(env);
  if !Store::locate(env).ours().is_empty() {
    found.push(Target::Cursor);
  }
  found
}

/// How many hosts a verb acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
  /// `install`: every chosen host.
  Many,
  /// `list`, `remove`, `update`: one host.
  One,
}

fn joined(targets: &[Target]) -> String {
  targets
    .iter()
    .map(|target| target.name())
    .collect::<Vec<_>>()
    .join(", ")
}

fn ambiguous(found: &[Target]) -> Fail {
  Fail::usage(format!(
    "several hosts found ({}). Choose one with --host.",
    joined(found)
  ))
}

/// The hosts to act on: `requested`, else the one on `PATH`, else a choice.
pub(crate) fn resolve(
  requested: Option<Target>,
  mode: Mode,
  env: &Env,
  ask: Option<&mut (dyn Ask + '_)>,
) -> Res<Vec<Target>> {
  if let Some(target) = requested {
    return Ok(vec![target]);
  }
  // Without a terminal only PATH hosts count, so scripts resolve as they always did.
  let found = match (&ask, mode) {
    (None, _any) => available(env),
    (Some(_ask), Mode::Many) => detected(env),
    (Some(_ask), Mode::One) => with_toolu(env),
  };
  match found.as_slice() {
    [] => Err(Fail::usage(format!(
      "no host found on PATH (probed {}). Choose one with --host.",
      joined(&PROBED)
    ))),
    [only] => Ok(vec![*only]),
    _several => {
      let Some(ask) = ask else {
        return Err(ambiguous(&found));
      };
      let labels: Vec<(String, Option<String>)> = found
        .iter()
        .map(|target| (target.name().to_owned(), Some(target.label().to_owned())))
        .collect();
      let chosen = match mode {
        Mode::Many => ask.many("Which hosts should receive the plugins?", &labels)?,
        Mode::One => vec![ask.one("Which host?", &labels)?],
      };
      let picked: Vec<Target> = chosen
        .into_iter()
        .filter_map(|at| found.get(at).copied())
        .collect();
      if picked.is_empty() {
        return Err(ambiguous(&found));
      }
      Ok(picked)
    }
  }
}

/// Claude Code takes every scope, the other hosts none.
pub(crate) fn check_scope(scope: Option<Scope>, target: Target) -> Res<()> {
  match (scope, target) {
    (Some(_), Target::Codex | Target::Muse | Target::Cursor) => Err(Fail::usage(format!(
      "--scope is for Claude Code; {} has no scope concept",
      target.name()
    ))),
    (None, _) | (Some(_), Target::Claude) => Ok(()),
  }
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
