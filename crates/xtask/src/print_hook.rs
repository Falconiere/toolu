//! `cargo xtask print-hook <plugin> <Event> <name> [--timeout N]`: the native
//! `hooks.json` entry to paste, from the launcher generator (#412).

use serde_json::json;
use toolu_protocol::launcher::{DEFAULT_TIMEOUT, LauncherHook, Target, hook};

use crate::options::Options;
use crate::{Verdict, output};

/// Print the entry for the plugin, event and hook name in `options.files`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let [plugin, event, name] = target_words(options, "print-hook")?;
  let timeout = match &options.timeout {
    Some(text) => text
      .parse()
      .map_err(|err| format!("--timeout must be a whole number of seconds, got {text}: {err}"))?,
    None => DEFAULT_TIMEOUT,
  };
  let generated = hook(
    &Target {
      plugin: &plugin,
      event: &event,
      name: &name,
    },
    timeout,
  )?;
  output::say(&entry_json(&generated));
  Ok(Verdict::Clean)
}

/// The plugin, event and hook name in `options.files`, for `task`'s messages;
/// the plugin must have a `plugins/<plugin>` directory.
pub(crate) fn target_words(options: &Options, task: &str) -> Result<[String; 3], String> {
  let words: Vec<String> = options
    .files
    .iter()
    .map(|word| word.to_string_lossy().into_owned())
    .collect();
  let Ok([plugin, event, name]) = <[String; 3]>::try_from(words) else {
    return Err(format!("{task} needs <plugin> <Event> <name>"));
  };
  if !options.root.join("plugins").join(&plugin).is_dir() {
    return Err(format!("no plugins/{plugin} directory"));
  }
  Ok([plugin, event, name])
}

/// The `hooks.json` hook object, pretty-printed.
pub(crate) fn entry_json(generated: &LauncherHook) -> String {
  let entry = json!({
    "type": "command",
    "command": generated.command,
    "commandWindows": generated.command_windows,
    "timeout": generated.timeout,
  });
  serde_json::to_string_pretty(&entry).unwrap_or_default()
}

#[cfg(test)]
#[path = "tests/print_hook_test.rs"]
mod tests;
