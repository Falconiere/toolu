//! The generated `hooks.json` launcher for native hooks (#412), checked by
//! `cargo xtask check-hooks`. `command` resolves `TOOLU_BIN`, else the install
//! directories, then `PATH`, keeping the first candidate whose `--hook-protocol`
//! prints an integer; enforcing events map any status but 0 or 2 to 2. Without a
//! binary an enforcing event fails closed and a context event prints the install
//! `systemMessage`. There is no Windows binary yet (#411), so `commandWindows`
//! reports the missing binary the same way. [`muse`] writes the same launcher
//! to a per-hook resolver file, since Muse hooks are argv, not shell strings.

use crate::event::is_enforcing;
use crate::install::missing_message;

pub mod muse;

/// The marker that makes a `hooks.json` command a native launcher entry.
pub const MARKER: &str = "--hook-protocol";

/// The `timeout` (seconds) `cargo xtask print-hook` writes by default.
pub const DEFAULT_TIMEOUT: u32 = 60;

/// The largest `timeout` (seconds) a native entry may declare.
pub const MAX_TIMEOUT: u32 = 600;

/// The `--plugin-root` value of a `hooks.json` launcher.
const CLAUDE_ROOT: &str = "\"${CLAUDE_PLUGIN_ROOT}\"";

/// The installed `toolu` candidates, in order, before `PATH`.
const INSTALL_DIRS: &str = "/opt/homebrew/bin/toolu /usr/local/bin/toolu \
  /home/linuxbrew/.linuxbrew/bin/toolu \"$HOME/.local/bin/toolu\"";

/// Which hook entry to launch.
#[derive(Debug, Clone, Copy)]
pub struct Target<'a> {
  /// Plugin directory name.
  pub plugin: &'a str,
  /// Host event the hook is registered under.
  pub event: &'a str,
  /// Hook name: `toolu [<plugin>] hook <name>`.
  pub name: &'a str,
}

/// One generated `hooks.json` hook.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LauncherHook {
  /// The POSIX `sh -c` command.
  pub command: String,
  /// Codex's cmd.exe override.
  pub command_windows: String,
  /// Host timeout in seconds.
  pub timeout: u32,
}

/// Generate the hook for `target`.
///
/// # Errors
/// When a name or the event fails its pattern, or `timeout` is outside 1..=600.
pub fn hook(target: &Target<'_>, timeout: u32) -> Result<LauncherHook, String> {
  validate(target)?;
  if !(1..=MAX_TIMEOUT).contains(&timeout) {
    return Err(format!(
      "timeout must be 1 to {MAX_TIMEOUT} seconds, got {timeout}"
    ));
  }
  Ok(LauncherHook {
    command: command(target, CLAUDE_ROOT),
    command_windows: command_windows(target),
    timeout,
  })
}

/// The hook name in a native `command`: the word after its first ` hook `.
pub fn hook_name(command: &str) -> Option<&str> {
  let (_, rest) = command.split_once(" hook ")?;
  rest.split_whitespace().next().filter(|name| is_name(name))
}

/// The argv after the binary: `hook <name>` for toolu, `<plugin> hook <name>` otherwise.
fn argv(target: &Target<'_>) -> String {
  if target.plugin == "toolu" {
    format!("hook {}", target.name)
  } else {
    format!("{} hook {}", target.plugin, target.name)
  }
}

/// The launcher for `target`, passing `root` (shell text) as `--plugin-root`.
fn command(target: &Target<'_>, root: &str) -> String {
  let run = format!(
    "{} --event {} --plugin-root {root}",
    argv(target),
    target.event
  );
  let missing = missing_message(target.plugin);
  let (found, absent) = if is_enforcing(target.event) {
    (
      format!(
        "if [ -n \"$t\" ]; then \"$t\" {run}; s=$?; case $s in 0|2) exit $s;; esac; \
         {}; exit 2; fi",
        abnormal_status(target.plugin)
      ),
      format!("printf '%s\\n' 'blocked: {missing}' >&2; exit 2"),
    )
  } else {
    (
      format!("if [ -n \"$t\" ]; then exec \"$t\" {run}; fi"),
      format!("printf '%s\\n' '{{\"systemMessage\":\"{missing}\"}}'; exit 0"),
    )
  };
  [
    "t=".to_owned(),
    format!(
      "if [ -n \"$TOOLU_BIN\" ]; then set -- \"$TOOLU_BIN\"; else set -- {INSTALL_DIRS} \
       \"$(command -v toolu 2>/dev/null)\"; fi"
    ),
    format!(
      "for c in \"$@\"; do if [ -n \"$c\" ] && [ -f \"$c\" ] && [ -x \"$c\" ]; then \
       case $(\"$c\" {MARKER} 2>/dev/null </dev/null) in ''|*[!0-9]*) ;; *) t=$c; break;; \
       esac; fi; done"
    ),
    found,
    absent,
  ]
  .join("; ")
}

/// Report only the observed shell status; it cannot identify a signal reliably.
fn abnormal_status(plugin: &str) -> String {
  format!(
    "printf '%s\\n' \"blocked: {plugin} plugin: toolu ended with status $s, so the action is \
     blocked. See docs/install.md.\" >&2"
  )
}

/// cmd.exe has no native `toolu` to run: report it as the POSIX command does without one.
fn command_windows(target: &Target<'_>) -> String {
  let missing = missing_message(target.plugin).replace('|', "^|");
  if is_enforcing(target.event) {
    format!("(1>&2 echo blocked: {missing}& exit /b 2)")
  } else {
    format!("echo {{\"systemMessage\":\"{missing}\"}}")
  }
}

fn validate(target: &Target<'_>) -> Result<(), String> {
  if !is_name(target.plugin) {
    return Err(format!(
      "plugin must match ^[a-z0-9]+(-[a-z0-9]+)*$: {:?}",
      target.plugin
    ));
  }
  if !is_name(target.name) {
    return Err(format!(
      "hook name must match ^[a-z0-9]+(-[a-z0-9]+)*$: {:?}",
      target.name
    ));
  }
  if !is_event(target.event) {
    return Err(format!(
      "event must match ^[A-Z][A-Za-z]+$: {:?}",
      target.event
    ));
  }
  Ok(())
}

/// Whether `text` is a valid plugin or hook name: `^[a-z0-9]+(-[a-z0-9]+)*$`.
pub fn is_name(text: &str) -> bool {
  !text.is_empty()
    && text.split('-').all(|part| {
      !part.is_empty()
        && part
          .bytes()
          .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// `^[A-Z][A-Za-z]+$`.
fn is_event(text: &str) -> bool {
  let mut bytes = text.bytes();
  bytes.next().is_some_and(|b| b.is_ascii_uppercase())
    && text.len() > 1
    && bytes.all(|b| b.is_ascii_alphabetic())
}

#[cfg(test)]
#[path = "tests/launcher_test.rs"]
mod tests;
