//! `.sh` modules (`runBash` and `moduleEnv` in `dispatch-bash.ts` and
//! `dispatch-walk.ts`): `bash <path>` fed the payload plus a newline, with the
//! module environment, its stdout read as `$(...)` reads it. Unlike TypeScript
//! a module has a deadline: past it the process group is killed and the status
//! is 124, so a timed-out module is never a deny.

use toolu_runtime::process::{Spec, Wait, run};

use crate::dispatch::MAX_OUTPUT_BYTES;
use crate::dispatch::ModuleResult;
use crate::dispatch::event::Payload;
use crate::dispatch::fold::Folded;
use crate::dispatch::output::substituted;
use crate::dispatch::session::Session;

use super::Entry;

/// The status of a module that ran past its deadline, as `timeout(1)` reports it.
pub(crate) const TIMED_OUT: i32 = 124;

/// The status a shell gives a command it cannot run.
const NOT_RUN: i32 = 127;

/// The environment `mod.sh` exports to its modules for `payload`.
fn module_env(payload: &Payload, session: &Session<'_>) -> toolu_runtime::env::Env {
  let config = session.config_root.to_string_lossy().into_owned();
  let lib = session.options.lib_dir.to_string_lossy().into_owned();
  let env = session
    .env
    .clone()
    .with("input", &payload.text)
    .with("tool_name", &payload.tool_name)
    .with("TOOLU_LIB_DIR", &lib)
    .with("TOOLU_CONFIG_DIR", &config);
  match &payload.edit {
    Some(edit) => env
      .with("TOOLU_EDIT_OPERATION", edit.operation.name())
      .with("TOOLU_EDIT_FROM", &edit.from)
      .with("TOOLU_EDIT_MOVED_TO", &edit.moved_to),
    None => env,
  }
}

/// Run one `.sh` module over `payload`.
pub(crate) fn run_executable(entry: &Entry, payload: &Payload, session: &Session<'_>) -> Folded {
  let mut spec = Spec::new(["bash".to_owned(), entry.path.to_string_lossy().into_owned()]);
  spec.cwd = Some(session.cwd().to_path_buf());
  spec.env = Some(module_env(payload, session));
  spec.stdin = format!("{}\n", payload.text).into_bytes();
  spec.timeout = session.options.module_timeout;
  spec.max_output_bytes = MAX_OUTPUT_BYTES;
  spec.wait = Wait::Streams;
  let (result, truncated) = match run(&spec) {
    Ok(output) if output.timed_out => (exit(TIMED_OUT, String::new()), false),
    Ok(output) => {
      let result = ModuleResult {
        stdout: substituted(&output.stdout).to_owned(),
        stderr: output.stderr,
        exit_code: output.exit_code,
      };
      (result, output.truncated)
    }
    Err(err) => (exit(NOT_RUN, format!("{err:?}\n")), false),
  };
  Folded {
    name: entry.file.clone(),
    result,
    truncated,
  }
}

fn exit(code: i32, stderr: String) -> ModuleResult {
  ModuleResult {
    stdout: String::new(),
    stderr,
    exit_code: code,
  }
}

#[cfg(test)]
#[path = "tests/executable_test.rs"]
mod tests;
