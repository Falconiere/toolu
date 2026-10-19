//! `cargo xtask print-muse-hook <plugin> <Event> <name> [--timeout-ms N] [--resolver]`:
//! the `.muse-plugin/plugin.json` hook entry to paste, or with `--resolver` the
//! text of its `hooks/muse/<name>` resolver file (#546).

use toolu_protocol::launcher::Target;
use toolu_protocol::launcher::muse::{DEFAULT_TIMEOUT_MS, hook};

use crate::options::Options;
use crate::print_hook::target_words;
use crate::{Verdict, output};

/// Print the entry, or the resolver, for the target in `options.files`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  output::say(&print(options)?);
  Ok(Verdict::Clean)
}

/// What `run` prints, without the final newline `output::say` adds.
pub(crate) fn print(options: &Options) -> Result<String, String> {
  let [plugin, event, name] = target_words(options, "print-muse-hook")?;
  let timeout_ms = match &options.timeout_ms {
    Some(text) => text.parse().map_err(|err| {
      format!("--timeout-ms must be a whole number of milliseconds, got {text}: {err}")
    })?,
    None => DEFAULT_TIMEOUT_MS,
  };
  let target = Target {
    plugin: &plugin,
    event: &event,
    name: &name,
  };
  let generated = hook(&target, timeout_ms)?;
  if options.resolver {
    return Ok(generated.resolver.trim_end_matches('\n').to_owned());
  }
  serde_json::to_string_pretty(&generated.entry).map_err(|err| format!("entry JSON: {err}"))
}

#[cfg(test)]
#[path = "tests/print_muse_hook_test.rs"]
mod tests;
