//! The plugin's native `SessionStart` hooks: advise how the agent's shell
//! runs native `toolu`, warn on Codex when a plugin the epic drives is
//! missing, and start the resident engine in process. Each prints at most one
//! output document and exits 0.

use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::startup::dependencies::{NoticeStyle, codex_dependency_notice};
use toolu_runtime::startup::native_toolu_hook_output;

/// The plugins an epic drives; Codex has no dependency field to install them.
const REQUIRED: [&str; 5] = [
  "toolu@toolu",
  "delivery-flow@toolu",
  "toolu-review@toolu",
  "pr-babysit@toolu",
  "brainstorm@toolu",
];

/// The longest engine failure detail in the message.
const DETAIL_CHARS: usize = 200;

fn print(stdout: Option<String>) -> Outcome {
  Outcome {
    exit: Exit::Success,
    stdout,
    stderr: None,
  }
}

/// The native-binary advice, once per session; silent when the agent's
/// shell already runs native `toolu`.
pub fn check_binary(env: &Env, stdin: Option<&str>) -> Outcome {
  print(native_toolu_hook_output(env, stdin))
}

/// On Codex, name each required plugin that is missing or disabled with its
/// install command; silent on other hosts and when all are present.
pub fn check_deps(env: &Env) -> Outcome {
  let roots = Roots::new(env.clone(), None);
  let notice = codex_dependency_notice(&REQUIRED, NoticeStyle::Each, &roots);
  print(notice.map(|text| text.trim_end_matches('\n').to_owned()))
}

/// Start the resident engine when an epic is registered and none runs;
/// a failure (a malformed `registry.json`) is one `systemMessage`.
pub fn engine_ensure(env: &Env) -> Outcome {
  let message = crate::control::ensure(env).err().map(|err| {
    let detail: String = err.trim().chars().take(DETAIL_CHARS).collect();
    serde_json::json!({"systemMessage": format!("epic engine: {detail}")}).to_string()
  });
  print(message)
}

#[cfg(test)]
#[path = "tests/hook_test.rs"]
mod tests;
