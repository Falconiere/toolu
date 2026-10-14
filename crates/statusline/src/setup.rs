//! Claude settings.json wiring, preserving custom bars and backing up changed files.

use std::fs::OpenOptions;
use std::io::{ErrorKind, Write as _};
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::host::roots::Roots;

const COMMAND: &str = "toolu statusline render";
const MARKER: &str = "statusline/statusline.sh";

/// A setup or migration result, with the existing first-word STATUS token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResultLine {
  pub exit: Exit,
  pub status: &'static str,
  pub message: String,
  pub settings: PathBuf,
}

fn result(exit: Exit, status: &'static str, message: String, settings: &Path) -> ResultLine {
  ResultLine {
    exit,
    status,
    message,
    settings: settings.to_path_buf(),
  }
}

fn settings_path(roots: &Roots) -> PathBuf {
  roots.config_root().join("settings.json")
}

fn load(path: &Path) -> Result<(Map<String, Value>, bool), String> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(error) if error.kind() == ErrorKind::NotFound => return Ok((Map::new(), true)),
    Err(error) => return Err(format!("could not read {}: {error}", path.display())),
  };
  if text.is_empty() {
    return Ok((Map::new(), true));
  }
  let parsed: Value = serde_json::from_str(&text).map_err(|error| {
    format!(
      "could not parse {}: {error} — not touching it",
      path.display()
    )
  })?;
  match parsed {
    Value::Object(map) => Ok((map, false)),
    Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) | Value::Array(_) => Err(
      format!("{} is not a JSON object — not touching it", path.display()),
    ),
  }
}

fn save(path: &Path, map: &Map<String, Value>) -> Result<(), String> {
  let parent = path
    .parent()
    .ok_or_else(|| format!("no parent for {}", path.display()))?;
  std::fs::create_dir_all(parent)
    .map_err(|error| format!("could not write {}: {error}", path.display()))?;
  let text = serde_json::to_string_pretty(map)
    .map_err(|error| format!("could not serialize {}: {error}", path.display()))?;
  let temp = path.with_extension(format!("json.{}.tmp", std::process::id()));
  let mut file = OpenOptions::new()
    .write(true)
    .create_new(true)
    .mode(0o600)
    .open(&temp)
    .map_err(|error| format!("could not write {}: {error}", path.display()))?;
  if let Err(error) = file.write_all(format!("{text}\n").as_bytes()) {
    let _removed = std::fs::remove_file(&temp);
    return Err(format!("could not write {}: {error}", path.display()));
  }
  let stored = (|| {
    if path.exists() {
      std::fs::copy(path, path.with_extension("json.bak"))
        .map_err(|error| format!("could not back up {}: {error}", path.display()))?;
    }
    std::fs::rename(&temp, path)
      .map_err(|error| format!("could not write {}: {error}", path.display()))
  })();
  if stored.is_err() {
    let _removed = std::fs::remove_file(&temp);
  }
  stored
}

fn owned_legacy(command: &str, roots: &Roots) -> bool {
  if !command.contains(MARKER)
    || ["|", ";", "&", "`", "$("]
      .iter()
      .any(|operator| command.contains(operator))
  {
    return false;
  }
  let words: Vec<&str> = command.split_whitespace().collect();
  let last = words
    .last()
    .map_or("", |word| word.trim_matches(['\'', '"'].as_slice()));
  let published = roots.config_root().join(MARKER);
  let known = last == "~/.claude/statusline/statusline.sh"
    || last == "$CLAUDE_CONFIG_DIR/statusline/statusline.sh"
    || last == published.to_string_lossy();
  if !known {
    return false;
  }
  let shell = |word: &str| ["bash", "sh", "zsh", "/bin/bash", "/bin/sh"].contains(&word);
  let option = |word: &str| ["-e", "--norc"].contains(&word);
  match words.as_slice() {
    [_] => true,
    [program, _] if shell(program) => true,
    [program, flag, _] if shell(program) && option(flag) => true,
    ["/usr/bin/env", program, _] if shell(program) => true,
    ["/usr/bin/env", program, flag, _] if shell(program) && option(flag) => true,
    _ => false,
  }
}

fn current_command(map: &Map<String, Value>) -> Option<&str> {
  map.get("statusLine")?.as_object()?.get("command")?.as_str()
}

/// Whether a settings command names only the former stable path.
fn legacy(map: &Map<String, Value>, roots: &Roots) -> bool {
  current_command(map).is_some_and(|command| owned_legacy(command, roots))
}

fn wire(map: &mut Map<String, Value>, path: &Path) -> Result<(), String> {
  map.insert(
    "statusLine".to_owned(),
    json!({"type":"command", "command":COMMAND}),
  );
  save(path, map)
}

/// Set up Claude's status bar, or refuse an existing custom command.
pub(crate) fn setup(roots: &Roots, force: bool) -> ResultLine {
  let path = settings_path(roots);
  let (mut map, created) = match load(&path) {
    Ok(value) => value,
    Err(error) => return result(Exit::Failure, "ERROR", format!("ERROR {error}"), &path),
  };
  let current = map.get("statusLine");
  if current_command(&map) == Some(COMMAND) {
    return result(
      Exit::Success,
      "ALREADY",
      "ALREADY statusLine already points at the statusline plugin — nothing to do.".to_owned(),
      &path,
    );
  }
  let was_legacy = legacy(&map, roots);
  if current.is_some_and(|value| !value.is_null()) && !was_legacy && !force {
    let message = format!(
      "REFUSED a different statusLine is already set in settings.json.\n  current: {}\n  re-run with --force to replace it, or set it manually:\n    \"statusLine\": {{\"type\":\"command\",\"command\":\"{COMMAND}\"}}",
      current.unwrap_or(&Value::Null)
    );
    return result(Exit::Blocked, "REFUSED", message, &path);
  }
  if let Err(error) = wire(&mut map, &path) {
    return result(Exit::Failure, "ERROR", format!("ERROR {error}"), &path);
  }
  let message = if created {
    format!(
      "CREATED wrote {} with the statusLine wired. Restart the session to see it.",
      path.display()
    )
  } else if was_legacy {
    format!(
      "WIRED updated statusLine in {} to run toolu statusline render directly (backup: settings.json.bak). Restart the session to see it.",
      path.display()
    )
  } else {
    format!(
      "WIRED added statusLine to {} (backup: settings.json.bak). Restart the session to see it.",
      path.display()
    )
  };
  result(
    Exit::Success,
    if created { "CREATED" } else { "WIRED" },
    message,
    &path,
  )
}

/// Migrate a legacy published path once. `None` means the setting was left untouched.
pub(crate) fn migrate(roots: &Roots) -> Result<Option<ResultLine>, String> {
  let path = settings_path(roots);
  let (mut map, _) = load(&path)?;
  if !legacy(&map, roots) {
    return Ok(None);
  }
  wire(&mut map, &path)?;
  Ok(Some(result(
    Exit::Success,
    "WIRED",
    format!(
      "statusline: migrated {} to toolu statusline render (backup: settings.json.bak)",
      path.display()
    ),
    &path,
  )))
}

/// Whether the settings' statusLine command still names the old stable path.
pub(crate) fn names_old_path(roots: &Roots) -> bool {
  load(&settings_path(roots))
    .is_ok_and(|(map, _)| current_command(&map).is_some_and(|command| command.contains(MARKER)))
}

/// CLI formatting: one JSON document or one newline-terminated STATUS line.
pub(crate) fn outcome(line: ResultLine, json_out: bool) -> Outcome {
  let text = if json_out {
    json!({
      "status": line.status,
      "message": line.message,
      "settings": line.settings.display().to_string()
    })
    .to_string()
  } else {
    line.message
  };
  Outcome {
    exit: line.exit,
    stdout: Some(text),
    stderr: None,
  }
}

/// Run `toolu statusline setup`.
pub(crate) fn run(roots: &Roots, force: bool, json_out: bool) -> Outcome {
  outcome(setup(roots, force), json_out)
}

#[cfg(test)]
#[path = "tests/setup_test.rs"]
mod tests;
