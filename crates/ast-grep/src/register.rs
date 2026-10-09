//! `SessionStart` manifest registration for the two compiled ast-grep rules.

use std::fs;
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
  let mut temporary =
    tempfile::NamedTempFile::new_in(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
  temporary
    .write_all(&body)
    .map_err(|error| error.to_string())?;
  temporary
    .write_all(b"\n")
    .map_err(|error| error.to_string())?;
  temporary
    .persist(dir.join(file))
    .map_err(|error| error.to_string())?;
  remove_old(&dir, name)
}

/// Write native manifests and remove this plugin's old JS and shell modules.
pub fn register(env: &Env, host: Host) -> Outcome {
  let roots = Roots::new(env.clone(), Some(host));
  let mut migrated = false;
  for &(event, name, matcher) in RULES {
    match write_one(&roots, event, name, matcher) {
      Ok(removed) => migrated |= removed,
      Err(error) => return Outcome::failed(Exit::Failure, error),
    }
  }
  Outcome {
    exit: Exit::Success,
    stdout: migrated.then(|| {
      serde_json::json!({
        "systemMessage": "ast-grep's Bun modules were replaced by native rules. Run wrapper commands with `toolu ast-grep search|files|scan|debug` and reports with `toolu ast-grep savings <ledger.jsonl>`."
      })
      .to_string()
    }),
    stderr: None,
  }
}

#[cfg(test)]
#[path = "tests/register_test.rs"]
mod tests;
