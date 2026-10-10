//! `toolu hook pre-tools` and `toolu hook post-tools` (#418): the engine's
//! dispatch over the built-in gates and the registry, inside the protocol's hook
//! main, so a panic or an unreadable payload blocks with exit 2 rather than
//! allowing. No `hooks.json` entry calls these until #425.

use std::collections::BTreeSet;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use toolu_engine::builtins::{POST_TOOL, PRE_TOOL};
use toolu_engine::dispatch::{DEFAULT_MODULE_TIMEOUT, dispatch};
use toolu_engine::{DispatchOptions, Phase as EnginePhase};
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::hook::{HookError, Io, Raw, Reply, run_hook_io};
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::registry::rule::Rule;

/// Which tool hook runs.
pub use toolu_engine::Phase;

/// The compiled-in rules a manifest may enable.
pub const RULES: &[&dyn Rule] = &[toolu_ast_grep::SEARCH_NUDGE, toolu_ast_grep::BYTE_SAVINGS];

/// The payload `crates/cli` read, as the stream the hook main reads it from.
struct Given(Result<Cursor<Vec<u8>>, Option<std::io::Error>>);

impl Read for Given {
  fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
    match &mut self.0 {
      Ok(cursor) => cursor.read(buf),
      Err(error) => Err(
        error
          .take()
          .unwrap_or_else(|| std::io::Error::other("the payload could not be read")),
      ),
    }
  }
}

/// One stream's bytes as an `Outcome` line: one trailing newline dropped, `None` when empty.
fn line(bytes: &[u8]) -> Option<String> {
  let text = String::from_utf8_lossy(bytes);
  let text = text.strip_suffix('\n').unwrap_or(&text);
  (!text.is_empty()).then(|| text.to_owned())
}

/// `<plugin root>/hooks/lib`: `--plugin-root`, else the host's plugin root variable.
fn lib_dir(plugin_root: Option<&Path>, roots: &Roots) -> PathBuf {
  plugin_root
    .map(Path::to_path_buf)
    .or_else(|| roots.plugin_root())
    .map(|root| root.join("hooks").join("lib"))
    .unwrap_or_default()
}

/// The hook's failure when the OS cannot give its working directory: the
/// modules would run in no directory, so the hook fails closed, as TypeScript
/// does when `process.cwd()` throws.
fn cwd_error(err: &std::io::Error) -> HookError {
  HookError::new(format!("the working directory could not be read: {err}"))
}

/// Run the `phase` tool hook over `payload`, as read from stdin.
pub fn tool_hook(
  phase: Phase,
  payload: std::io::Result<String>,
  plugin_root: Option<&Path>,
) -> Outcome {
  run(phase, payload, plugin_root, false)
}

/// Run only the ast-grep manifest rules for a host that dispatches core gates itself.
pub fn ast_grep_tool_hook(
  phase: Phase,
  payload: std::io::Result<String>,
  plugin_root: Option<&Path>,
) -> Outcome {
  run(phase, payload, plugin_root, true)
}

fn run(
  phase: Phase,
  payload: std::io::Result<String>,
  plugin_root: Option<&Path>,
  ast_only: bool,
) -> Outcome {
  let env = Env::process();
  let host = detect(&env, None).host;
  let lib = lib_dir(plugin_root, &Roots::new(env.clone(), Some(host)));
  let cwd = current_dir();
  let selected = BTreeSet::from(["ast-grep@toolu".to_owned()]);
  let options = cwd.as_ref().map_err(cwd_error).map(|cwd| DispatchOptions {
    env: &env,
    cwd,
    lib_dir: &lib,
    builtins: match (ast_only, phase) {
      (true, _) => &[],
      (false, EnginePhase::Pre) => PRE_TOOL,
      (false, EnginePhase::Post) => POST_TOOL,
    },
    rules: RULES,
    selected_specs: ast_only.then_some(&selected),
    continue_post_blocks: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  });
  hook_main(phase, payload, host, options.as_ref().map_err(Clone::clone))
}

/// The dispatch inside `run_hook_io`, over in-memory streams; an `options`
/// error fails the hook once its payload is read.
fn hook_main(
  phase: Phase,
  payload: std::io::Result<String>,
  host: Host,
  options: Result<&DispatchOptions<'_>, HookError>,
) -> Outcome {
  let event = match phase {
    EnginePhase::Pre => HostEvent::ToolPre,
    EnginePhase::Post => HostEvent::ToolPost,
  };
  let stdin = Given(
    payload
      .map(|text| Cursor::new(text.into_bytes()))
      .map_err(Some),
  );
  let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
  let io = Io {
    stdin,
    stdout: &mut stdout,
    stderr: &mut stderr,
  };
  let code = run_hook_io(io, event, host, |text| {
    let result = dispatch(phase, text, options?).result;
    let exit = if result.exit_code == 2 {
      Exit::Blocked
    } else {
      Exit::Success
    };
    Ok(Reply::Raw(Raw {
      stdout: result.stdout,
      stderr: result.stderr,
      exit,
    }))
  });
  Outcome {
    exit: if code == ExitCode::from(2) {
      Exit::Blocked
    } else {
      Exit::Success
    },
    stdout: line(&stdout),
    stderr: line(&stderr),
  }
}

#[cfg(test)]
#[path = "tests/tool_hook_test.rs"]
mod tests;
