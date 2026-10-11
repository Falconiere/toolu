//! The Bun `hooks.json` command pair from `@toolu/core/launcher`.

use regex::Regex;

/// POSIX `command` and Codex `commandWindows` for one bundle entry.
#[derive(Debug)]
pub(crate) struct Launch {
  pub(crate) command: String,
  pub(crate) windows: String,
}

/// Build the launcher pair, or the validation message `launcher.ts` throws.
pub(crate) fn launch(plugin: &str, event: &str, entry: &str) -> Result<Launch, String> {
  if !ident(entry) {
    return Err(format!(
      "launcher entry must match [a-z0-9]+(?:-[a-z0-9]+)*: {}",
      json_str(entry)
    ));
  }
  if !ident(plugin) {
    return Err(format!(
      "launcher plugin must match [a-z0-9]+(?:-[a-z0-9]+)*: {}",
      json_str(plugin)
    ));
  }
  Ok(Launch {
    command: posix(plugin, event, entry),
    windows: windows(plugin, event, entry),
  })
}

/// A hand-written `toolu hook` that is not the generated native command.
pub(crate) fn is_native_like(command: &str) -> bool {
  matches(r"\btoolu(?:\s+[a-z0-9-]+)?\s+hook\s+[a-z0-9-]+", command)
    || matches(r"\bhook\s+[a-z0-9-]+\s+--event\s+[A-Z][A-Za-z]+", command)
}

/// A Bun launcher: a Windows command, a `hooks/dist/` path, or the word `bun`.
pub(crate) fn is_launcher(command: &str, has_windows: bool) -> bool {
  has_windows || command.contains("hooks/dist/") || matches(r"\bbun\b", command)
}

/// The bundle stem in `hooks/dist/<entry>.js`, from `command` then `windows`.
pub(crate) fn bundle_name(command: &str, windows: &str) -> Option<String> {
  bundle_in(command).or_else(|| bundle_in(windows))
}

fn posix(plugin: &str, event: &str, entry: &str) -> String {
  let bundle = format!(r#""${{CLAUDE_PLUGIN_ROOT}}/hooks/dist/{entry}.js""#);
  let scan = r#"for c in "$TOOLU_BUN" "$(command -v bun 2>/dev/null)" "$HOME/.bun/bin/bun"; do if [ -n "$c" ] && [ -f "$c" ] && [ -x "$c" ]; then b=$c; break; fi; done"#;
  let launch = format!(r#"if [ -n "$b" ]; then exec "$b" {bundle}; fi"#);
  let missing = missing_posix(plugin, event);
  format!("b=; {scan}; {launch}; {missing}")
}

fn missing_posix(plugin: &str, event: &str) -> String {
  if enforcing(event) {
    return format!(
      r"printf '%s\n' 'blocked: {}' >&2; exit 2",
      missing_message(plugin)
    );
  }
  format!(r"printf '%s\n' '{}'; exit 0", advisory_json(plugin))
}

fn windows(plugin: &str, event: &str, entry: &str) -> String {
  let bundle = format!(r#""%PLUGIN_ROOT%\hooks\dist\{entry}.js""#);
  let home = r#""%USERPROFILE%\.bun\bin\bun.exe""#;
  let missing = missing_windows(plugin, event);
  format!(
    r#"if exist "%TOOLU_BUN%" ("%TOOLU_BUN%" {bundle}) else (where /q bun& if not errorlevel 1 (bun {bundle}) else if exist {home} ({home} {bundle}) else {missing})"#
  )
}

fn missing_windows(plugin: &str, event: &str) -> String {
  if enforcing(event) {
    return format!(
      r"(1>&2 echo blocked: {}& exit /b 2)",
      missing_message(plugin)
    );
  }
  format!("(echo {})", advisory_json(plugin))
}

fn missing_message(plugin: &str) -> String {
  format!(
    "{plugin} plugin: Bun runtime not found, checked TOOLU_BUN, PATH and ~/.bun/bin/bun. \
     Install Bun 1.4.x from https://bun.sh and restart the session. See docs/runtime.md."
  )
}

fn advisory_json(plugin: &str) -> String {
  format!("{{\"systemMessage\":\"{}\"}}", missing_message(plugin))
}

fn enforcing(event: &str) -> bool {
  event == "PreToolUse" || event == "PermissionRequest"
}

fn ident(text: &str) -> bool {
  matches(r"^[a-z0-9]+(?:-[a-z0-9]+)*$", text)
}

fn bundle_in(command: &str) -> Option<String> {
  let pattern = Regex::new(r#"hooks[/\\]dist[/\\]([^"'\s/\\]+)\.js"#).ok()?;
  pattern
    .captures(command)
    .and_then(|caps| caps.get(1))
    .map(|name| name.as_str().to_owned())
}

fn matches(pattern: &str, text: &str) -> bool {
  Regex::new(pattern).is_ok_and(|expr| expr.is_match(text))
}

fn json_str(text: &str) -> String {
  serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_owned())
}

#[cfg(test)]
#[path = "tests/bun_launcher_test.rs"]
mod tests;
