//! Hand-parsed argv until #442 replaces it with clap, keeping these forms.

/// The usage text printed with exit 64.
pub(crate) const USAGE: &str = "usage: toolu --version | toolu --hook-protocol | \
  toolu [<plugin>] hook <name> [--event <Event>] [--plugin-root <dir>]";

/// A parsed command line.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Command {
  /// `toolu --version`.
  Version,
  /// `toolu --hook-protocol`: the native signature the launcher checks.
  HookProtocol,
  /// `toolu [<plugin>] hook <name> …`.
  Hook(HookRequest),
}

/// One hook run.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HookRequest {
  /// The plugin namespace; `toolu` when the argv starts with `hook`.
  pub(crate) plugin: String,
  /// The hook name.
  pub(crate) name: String,
  /// The host event (`--event`); absent counts as enforcing.
  pub(crate) event: Option<String>,
  /// The calling plugin's root (`--plugin-root`); absent skips the skew prelude.
  pub(crate) plugin_root: Option<String>,
}

/// Parse argv after the program name.
pub(crate) fn parse(words: &[String]) -> Result<Command, String> {
  let strs: Vec<&str> = words.iter().map(String::as_str).collect();
  match strs.as_slice() {
    ["--version"] => Ok(Command::Version),
    ["--hook-protocol"] => Ok(Command::HookProtocol),
    ["hook", name, rest @ ..] => hook("toolu", name, rest),
    [plugin, "hook", name, rest @ ..] if !plugin.starts_with('-') => hook(plugin, name, rest),
    [] => Err("no command".to_owned()),
    _ => Err(format!("unknown command: {}", words.join(" "))),
  }
}

fn hook(plugin: &str, name: &str, flags: &[&str]) -> Result<Command, String> {
  if name.starts_with('-') {
    return Err(format!("hook needs a name, got {name}"));
  }
  let mut request = HookRequest {
    plugin: plugin.to_owned(),
    name: name.to_owned(),
    event: None,
    plugin_root: None,
  };
  let mut rest = flags.iter();
  while let Some(flag) = rest.next() {
    let value = rest.next().ok_or_else(|| format!("{flag} needs a value"))?;
    let slot = match *flag {
      "--event" => &mut request.event,
      "--plugin-root" => &mut request.plugin_root,
      _ => return Err(format!("unknown flag {flag}")),
    };
    *slot = Some((*value).to_owned());
  }
  Ok(Command::Hook(request))
}

#[cfg(test)]
#[path = "tests/args_test.rs"]
mod tests;
