//! `toolu hook pre-tools` and `toolu hook post-tools` (#418): the engine's
//! dispatch over the built-in gates and the registry, inside the protocol's hook
//! main, so a panic or an unreadable payload blocks with exit 2 rather than
//! allowing. No `hooks.json` entry calls these until #425.

use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use toolu_engine::builtins::{POST_TOOL, PRE_TOOL};
use toolu_engine::dispatch::{DEFAULT_MODULE_TIMEOUT, dispatch};
use toolu_engine::{DispatchOptions, Phase as EnginePhase};
use toolu_protocol::event::HostEvent;
use toolu_protocol::exit::Exit;
use toolu_protocol::hook::{Io, Raw, Reply, run_hook_io};
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::detect::detect;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_dir;
use toolu_runtime::registry::rule::Rule;

/// Which tool hook runs.
pub use toolu_engine::Phase;

/// The compiled-in rules a manifest may enable; the rule crates add theirs (#426–#429).
pub const RULES: &[&dyn Rule] = &[];

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

/// Run the `phase` tool hook over `payload`, as read from stdin.
pub fn tool_hook(
  phase: Phase,
  payload: std::io::Result<String>,
  plugin_root: Option<&Path>,
) -> Outcome {
  let env = Env::process();
  let cwd = current_dir();
  let host = detect(&env, None).host;
  let lib = lib_dir(plugin_root, &Roots::new(env.clone(), Some(host)));
  let options = DispatchOptions {
    env: &env,
    cwd: &cwd,
    lib_dir: &lib,
    builtins: match phase {
      EnginePhase::Pre => PRE_TOOL,
      EnginePhase::Post => POST_TOOL,
    },
    rules: RULES,
    selected_specs: None,
    continue_post_blocks: false,
    module_timeout: DEFAULT_MODULE_TIMEOUT,
  };
  hook_main(phase, payload, host, &options)
}

/// The dispatch inside `run_hook_io`, over in-memory streams.
fn hook_main(
  phase: Phase,
  payload: std::io::Result<String>,
  host: Host,
  options: &DispatchOptions<'_>,
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
    let result = dispatch(phase, text, options).result;
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
