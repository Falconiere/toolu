//! `SessionStart` manifest registration for the two compiled ast-grep rules.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write as _};
use std::path::Path;

use toolu_protocol::exit::Exit;
use toolu_protocol::host::Host;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::manifest::ModuleManifest;
use toolu_runtime::registry::{ModuleKind, RegistryEvent, event_dir, file_name};

const SPEC: &str = "ast-grep@toolu";
const RULES: &[(RegistryEvent, &str, &str)] = &[
  (RegistryEvent::ToolPre, "search-nudge", "Grep|Bash|Shell"),
  (
    RegistryEvent::ToolPost,
    "byte-savings",
    "Read|Grep|Glob|Bash|Shell",
  ),
];

fn remove_old(dir: &Path, name: &str) -> Result<bool, String> {
  let mut removed = false;
  for kind in [ModuleKind::Esm, ModuleKind::Bash] {
    let file = file_name(SPEC, name, kind).map_err(|error| error.0)?;
    match fs::remove_file(dir.join(file)) {
      Ok(()) => removed = true,
      Err(error) if error.kind() == ErrorKind::NotFound => {}
      Err(error) => return Err(format!("ast-grep registration: {error}")),
    }
  }
  Ok(removed)
}

fn write_one(
  roots: &Roots,
  event: RegistryEvent,
  name: &str,
  matcher: &str,
) -> Result<bool, String> {
  let dir = event_dir(roots, event);
  fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
  let file = file_name(SPEC, name, ModuleKind::Manifest).map_err(|error| error.0)?;
  let manifest = ModuleManifest {
    version: 1,
    spec: SPEC.to_owned(),
    name: name.to_owned(),
    event,
    matcher: matcher.to_owned(),
  };
  let body = serde_json::to_vec(&manifest).map_err(|error| error.to_string())?;
  let mut body = body;
  body.push(b'\n');
  if fs::read(dir.join(&file)).is_ok_and(|existing| existing == body) {
    return remove_old(&dir, name);
  }
  let mut temporary =
    tempfile::NamedTempFile::new_in(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
  temporary
    .write_all(&body)
    .map_err(|error| error.to_string())?;
  temporary
    .persist(dir.join(file))
    .map_err(|error| error.to_string())?;
  remove_old(&dir, name)
}

fn report_one(
  env: &Env,
  roots: &Roots,
  event: RegistryEvent,
  name: &str,
  matcher: &str,
) -> Result<(), String> {
  let Some(report) = env.get("TOOLU_STARTUP_REPORT") else {
    return Ok(());
  };
  let file = file_name(SPEC, name, ModuleKind::Manifest).map_err(|error| error.0)?;
  let target = event_dir(roots, event).join(file);
  let record = serde_json::json!({
    "kind": "native-registry",
    "spec": SPEC,
    "name": name,
    "event": event,
    "matcher": matcher,
    "target": target,
  });
  let mut output = OpenOptions::new()
    .append(true)
    .open(report)
    .map_err(|error| format!("cannot open startup report {report}: {error}"))?;
  let mut line = record.to_string().into_bytes();
  line.push(b'\n');
  output
    .write_all(&line)
    .map_err(|error| format!("cannot write startup report {report}: {error}"))
}

/// Write native manifests and remove this plugin's old JS and shell modules.
/// Never fails the session: an error is a `systemMessage`, since Codex shows a
/// failed `SessionStart` hook without its stderr (#642).
pub fn register(env: &Env, host: Host) -> Outcome {
  let message = match sync(env, &Roots::new(env.clone(), Some(host))) {
    Ok(true) => Some("ast-grep's Bun modules were replaced by native rules. Run wrapper commands with `toolu ast-grep search|files|scan|debug` and reports with `toolu ast-grep savings <ledger.jsonl>`.".to_owned()),
    Ok(false) => None,
    Err(error) => Some(format!("ast-grep registration failed: {error}")),
  };
  Outcome {
    exit: Exit::Success,
    stdout: message.map(|text| serde_json::json!({ "systemMessage": text }).to_string()),
    stderr: None,
  }
}

/// Write and report each rule's manifest; whether an old module was removed.
fn sync(env: &Env, roots: &Roots) -> Result<bool, String> {
  let mut migrated = false;
  for &(event, name, matcher) in RULES {
    migrated |= write_one(roots, event, name, matcher)?;
    report_one(env, roots, event, name, matcher)?;
  }
  Ok(migrated)
}

#[cfg(test)]
#[path = "tests/register_test.rs"]
mod tests;
