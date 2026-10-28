//! The hosts the installer targets and how `install`, `list` and `remove` choose
//! one: an explicit `--host` always wins, a single host on `PATH` is used
//! without asking, and several are never resolved silently (a terminal asks,
//! otherwise the run refuses). At a terminal, install offers every supported
//! host on the machine, Cursor included when `~/.cursor` or `cursor-agent` is
//! there and `OpenCode` when its config directory or `opencode` is. `update`
//! without `--host` asks nothing: it takes `with_toolu`.

use std::path::Path;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use crate::args::Scope;
use crate::fail::{Fail, Res};
use crate::local::store::Store;
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
  /// `OpenCode` V2, through its plugins directory and a generated shim.
  Opencode,
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
      Target::Opencode => "opencode",
    }
  }

  /// How the host picker names the host.
  fn label(self) -> &'static str {
    match self {
      Target::Claude => "Claude Code",
      Target::Codex => "Codex",
      Target::Muse => "Muse",
      Target::Cursor => "Cursor",
      Target::Opencode => "OpenCode",
    }
  }

  /// Whether removal on this host reaches only toolu's plugins
  /// (`<name>@toolu`, or the install marker of a copied directory), so `remove` may hand it the
  /// name of a plugin the marketplace retired (#615). Muse removes by bare
  /// name, from any source. A new target must decide here.
  pub(crate) fn removes_only_toolu(self) -> bool {
    match self {
      Target::Claude | Target::Codex | Target::Cursor | Target::Opencode => true,
      Target::Muse => false,
    }
  }
}

/// The installer target `--host` names; Hermes is not a target yet. Without a
/// terminal Cursor and `OpenCode` are never detected (a machine with one of them
/// and another host would stop resolving silently), so a script names them with
/// `--host cursor` or `--host opencode`.
pub(crate) fn target_of(host: Host) -> Res<Target> {
  match host {
    Host::Claude => Ok(Target::Claude),
    Host::Codex => Ok(Target::Codex),
    Host::Muse => Ok(Target::Muse),
    Host::Cursor => Ok(Target::Cursor),
    Host::Opencode => Ok(Target::Opencode),
    Host::Hermes => Err(Fail::usage(format!(
      "--host {} is not an installer target; toolu plugins installs into claude, codex, muse, cursor or opencode",
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

/// Every supported host on this machine: those on `PATH`, then Cursor when
/// `~/.cursor` exists or `cursor-agent` is on `PATH`, then `OpenCode` when its
/// config directory exists or `opencode` is on `PATH` (install's picker).
pub(crate) fn detected(env: &Env) -> Vec<Target> {
  let mut found = available(env);
  // `~/.cursor` itself: `TOOLU_CONFIG_DIR` would answer for every host.
  if env.home().join(".cursor").is_dir() || on_path("cursor-agent", env) {
    found.push(Target::Cursor);
  }
  if opencode_config(env).is_dir() || on_path("opencode", env) {
    found.push(Target::Opencode);
  }
  found
}

/// `OpenCode`'s global config directory, read without `TOOLU_CONFIG_DIR`.
fn opencode_config(env: &Env) -> std::path::PathBuf {
  env
    .get("XDG_CONFIG_HOME")
    .map_or_else(|| env.home().join(".config"), std::path::PathBuf::from)
    .join("opencode")
}

/// The hosts on `PATH`, then Cursor and `OpenCode` when toolu installed a plugin
/// there, so a leftover `~/.cursor` never asks which host `list`, `remove` or
/// `update` means.
pub(crate) fn with_toolu(env: &Env) -> Vec<Target> {
  let mut found = available(env);
  for target in [Target::Cursor, Target::Opencode] {
    if !Store::locate(env, target).ours().is_empty() {
      found.push(target);
    }
  }
  found
}

/// How many hosts a verb acts on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
  /// `install`: every chosen host.
  Many,
  /// `list`, `remove`: one host.
  One,
}

fn joined(targets: &[Target]) -> String {
  targets
    .iter()
    .map(|target| target.name())
    .collect::<Vec<_>>()
    .join(", ")
}

/// The refusal when no host is on `PATH`.
pub(crate) fn none_found() -> Fail {
  Fail::usage(format!(
    "no host found on PATH (probed {}). Choose one with --host.",
    joined(&PROBED)
  ))
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
    [] => Err(none_found()),
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
    (Some(_), Target::Codex | Target::Muse | Target::Cursor | Target::Opencode) => {
      Err(Fail::usage(format!(
        "--scope is for Claude Code; {} has no scope concept",
        target.name()
      )))
    }
    (None, _) | (Some(_), Target::Claude) => Ok(()),
  }
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
