//! The `PreToolUse` and `PostToolUse` dispatchers (`dispatchPreTool` and
//! `dispatchPostTool` in `packages/toolu-core/src/dispatch/dispatch.ts`). An edit
//! tool is walked once per affected path (a deny, block or exit 2 on any path
//! wins for the whole patch); everything else is walked once. Each walk runs the
//! built-in gates, then the registry.

mod edits;
pub(crate) mod event;
pub(crate) mod fold;
pub(crate) mod output;
pub(crate) mod session;
pub(crate) mod walk;

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::Rule;

use crate::gate::Gate;
use crate::trace::Step;

/// How long one `.sh` module, or one `.js` module of a Bun batch, may run.
pub const DEFAULT_MODULE_TIMEOUT: Duration = Duration::from_secs(30);

/// The bytes of stdout and stderr together kept from one module process.
pub const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

/// Which hook a dispatch serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
  /// `PreToolUse`: before the tool runs.
  Pre,
  /// `PostToolUse`: after it ran.
  Post,
}

/// What stays fixed for one hook call.
#[derive(Clone, Copy)]
pub struct DispatchOptions<'a> {
  /// The hook's environment.
  pub env: &'a Env,
  /// The hook process's working directory.
  pub cwd: &'a Path,
  /// `TOOLU_LIB_DIR` for `.sh` modules: `<plugin root>/hooks/lib`.
  pub lib_dir: &'a Path,
  /// Built-in gates, run first in table order.
  pub builtins: &'a [&'a dyn Gate],
  /// Compiled-in rules a manifest may enable.
  pub rules: &'a [&'a dyn Rule],
  /// A host-resolved selection of plugin specs (`selectedRegistrySpecs`).
  pub selected_specs: Option<&'a BTreeSet<String>>,
  /// After a tool, check every patch path past a block (`continuePostBlocks`).
  pub continue_post_blocks: bool,
  /// How long one module may run.
  pub module_timeout: Duration,
}

/// What the host must see: stdout, stderr and an exit status of 0 or 2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ModuleResult {
  /// The hook's stdout.
  pub stdout: String,
  /// The hook's stderr.
  pub stderr: String,
  /// 0, or 2 to block.
  pub exit_code: i32,
}

/// One dispatch: the hook's result and every module step it reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dispatched {
  /// What the host sees.
  pub result: ModuleResult,
  /// Each module reached, in order, across every walk of the call.
  pub trace: Vec<Step>,
}

/// One `PreToolUse` hook call: the host's stdin in, what the host must see out.
pub fn dispatch_pre_tool(stdin: &str, options: &DispatchOptions<'_>) -> Dispatched {
  dispatch(Phase::Pre, stdin, options)
}

/// One `PostToolUse` hook call: the host's stdin in, what the host must see out.
pub fn dispatch_post_tool(stdin: &str, options: &DispatchOptions<'_>) -> Dispatched {
  dispatch(Phase::Post, stdin, options)
}

/// One hook call of `phase`.
pub fn dispatch(phase: Phase, stdin: &str, options: &DispatchOptions<'_>) -> Dispatched {
  let mut trace = Vec::new();
  let result = match session::Session::open(phase, options) {
    session::Opened::Disabled(warnings) => ModuleResult {
      stderr: warnings,
      ..ModuleResult::default()
    },
    session::Opened::Ready(session, warnings) => {
      let result = edits::dispatch_input(output::substituted(stdin), &session, &mut trace);
      ModuleResult {
        stderr: format!("{warnings}{}", result.stderr),
        ..result
      }
    }
  };
  Dispatched { result, trace }
}

#[cfg(test)]
#[path = "tests/dispatch_test.rs"]
mod tests;
