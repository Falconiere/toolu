//! The generated `hooks.json` launcher for native hooks (#412), checked by
//! `cargo xtask check-hooks`. `command` resolves `TOOLU_BIN`, else the install
//! directories, then `PATH`, keeping the first candidate whose `--hook-protocol`
//! prints an integer; enforcing events map any status but 0 or 2 to 2. Without a
//! binary it runs the Bun bundle (#425) or fails closed. `commandWindows` keeps
//! the Bun chain (#411: no Windows binary yet).

use crate::event::is_enforcing;
use crate::install::{fallback_advisory, missing_message};

/// The marker that makes a `hooks.json` command a native launcher entry.
pub const MARKER: &str = "--hook-protocol";

/// The `timeout` (seconds) `cargo xtask print-hook` writes by default.
pub const DEFAULT_TIMEOUT: u32 = 60;

/// The largest `timeout` (seconds) a native entry may declare.
pub const MAX_TIMEOUT: u32 = 600;

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
  /// Hook name: `toolu [<plugin>] hook <name>`, and `hooks/dist/<name>.js`.
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
    command: command(target),
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

fn command(target: &Target<'_>) -> String {
  let run = format!(
    "{} --event {} --plugin-root \"${{CLAUDE_PLUGIN_ROOT}}\"",
    argv(target),
    target.event
  );
  let bundle = format!("\"${{CLAUDE_PLUGIN_ROOT}}/hooks/dist/{}.js\"", target.name);
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
  let (lookup, fallback_guard) = native_lookup(target, &found);
  [
    "t=".to_owned(),
    lookup,
    format!(
      "if {fallback_guard}; then b=; for c in \"$TOOLU_BUN\" \
       \"$(command -v bun 2>/dev/null)\" \"$HOME/.bun/bin/bun\"; do if [ -n \"$c\" ] && \
       [ -f \"$c\" ] && [ -x \"$c\" ]; then b=$c; break; fi; done; if [ -n \"$b\" ] && \
       [ -f {bundle} ]; then {}; exec \"$b\" {bundle}; fi; fi",
      fallback_notice(target)
    ),
    absent,
  ]
  .join("; ")
}

/// A signal-shaped shell status may also be an ordinary exit code, so name it as possible.
fn abnormal_status(plugin: &str) -> String {
  if plugin != "toolu" {
    return format!(
      "printf '%s\\n' \"blocked: {plugin} plugin: toolu ended with status $s, so the action is \
       blocked. See docs/install.md.\" >&2"
    );
  }
  "if [ \"$s\" -gt 128 ] && [ \"$s\" -le 192 ]; then \
     printf '%s\\n' \"blocked: toolu plugin: toolu may have terminated on signal \
     $((s - 128)) (status $s), so the action is blocked. See docs/install.md.\" >&2; \
     else printf '%s\\n' \"blocked: toolu plugin: toolu exited with status $s, so the action \
     is blocked. See docs/install.md.\" >&2; fi"
    .to_owned()
}

/// Find a native binary unless toolu's transition rollback selects Bun.
fn native_lookup(target: &Target<'_>, found: &str) -> (String, &'static str) {
  let lookup = [
    format!(
      "if [ -n \"$TOOLU_BIN\" ]; then set -- \"$TOOLU_BIN\"; else set -- {INSTALL_DIRS} \
       \"$(command -v toolu 2>/dev/null)\"; fi"
    ),
    format!(
      "for c in \"$@\"; do if [ -n \"$c\" ] && [ -f \"$c\" ] && [ -x \"$c\" ]; then \
       case $(\"$c\" {MARKER} 2>/dev/null </dev/null) in ''|*[!0-9]*) ;; *) t=$c; break;; \
       esac; fi; done"
    ),
    found.to_owned(),
  ]
  .join("; ");
  if target.plugin == "toolu" {
    (
      format!("if [ \"$TOOLU_RUNTIME\" != bun ]; then {lookup}; fi"),
      "[ \"$TOOLU_RUNTIME\" = bun ] || [ -z \"$TOOLU_BIN\" ]",
    )
  } else {
    (lookup, "[ -z \"$TOOLU_BIN\" ]")
  }
}

fn fallback_notice(target: &Target<'_>) -> String {
  let missing = format!("printf '%s\\n' '{}' >&2", fallback_advisory(target.plugin));
  if target.plugin == "toolu" {
    format!(
      "if [ \"$TOOLU_RUNTIME\" = bun ]; then printf '%s\\n' \
       'toolu plugin: TOOLU_RUNTIME=bun selected the transition Bun bundle.' >&2; \
       else {missing}; fi"
    )
  } else {
    missing
  }
}

fn command_windows(target: &Target<'_>) -> String {
  let bundle = format!("\"%PLUGIN_ROOT%\\hooks\\dist\\{}.js\"", target.name);
  let home = "\"%USERPROFILE%\\.bun\\bin\\bun.exe\"";
  let missing = missing_message(target.plugin).replace('|', "^|");
  let absent = if is_enforcing(target.event) {
    format!("(1>&2 echo blocked: {missing}& exit /b 2)")
  } else {
    format!("(echo {{\"systemMessage\":\"{missing}\"}})")
  };
  format!(
    "if exist \"%TOOLU_BUN%\" (\"%TOOLU_BUN%\" {bundle}) else \
     (where /q bun& if not errorlevel 1 (bun {bundle}) else \
     if exist {home} ({home} {bundle}) else {absent})"
  )
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
