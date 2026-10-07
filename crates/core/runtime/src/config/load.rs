//! The config loader (`config-files.ts`, `config-load.ts`): reads the user and
//! project `toolu.config.json`, deep-merges them like jq `$u * $p` (project
//! wins) and checks each file's envelope. Malformed JSON is ignored with a
//! warning, as in bash. An envelope toolu cannot understand (a non-object, an
//! unknown top-level key, a `version` other than 1) marks the config invalid:
//! `data` is empty and every gate fails closed.

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use toolu_protocol::host::Host;

use crate::host::roots::Roots;
use crate::json::{stringify, top_level_keys};

/// The prefix a caller puts before each config warning on stderr.
pub const WARN_PREFIX: &str = "toolu-config: ";

/// The top-level keys of a v1 config. `epic` is a namespaced section (#463)
/// whose contents the loader never inspects.
const KNOWN_KEYS: &[&str] = &[
  "version",
  "skills",
  "hooks",
  "mcp",
  "agents",
  "models",
  "lang",
  "docsSync",
  "telemetry",
  "agentTier",
  "planLedger",
  "gates",
  "permissions",
  "projectSkills",
  "prBabysit",
  "comemory",
  "epic",
];

/// Where the two config files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigFiles {
  /// `<TOOLU_USER_CONFIG_DIR or config root>/toolu.config.json`.
  pub user: PathBuf,
  /// `<project>/<dirname>/toolu.config.json`, `None` outside a project.
  pub project: Option<PathBuf>,
}

/// The merged config and what the loader and resolvers had to say about it.
#[derive(Debug)]
pub struct LoadedConfig {
  /// The deep-merged user and project config; empty when `invalid` is set.
  pub data: Map<String, Value>,
  /// Why an envelope was rejected (`<path>: <reason>`); set means fail closed.
  pub invalid: Option<String>,
  /// The files read.
  pub files: ConfigFiles,
  /// The host the files were resolved for.
  pub host: Host,
  warnings: RefCell<Vec<String>>,
}

impl LoadedConfig {
  /// A valid config holding `data` for `host`, read from no file: what a host
  /// that merges the config itself passes in.
  pub fn from_data(data: Map<String, Value>, host: Host) -> LoadedConfig {
    let files = ConfigFiles {
      user: PathBuf::new(),
      project: None,
    };
    LoadedConfig {
      data,
      invalid: None,
      files,
      host,
      warnings: RefCell::new(Vec::new()),
    }
  }

  /// Records a warning for the caller to print after [`WARN_PREFIX`].
  pub fn warn(&self, message: String) {
    self.warnings.borrow_mut().push(message);
  }

  /// The warnings recorded so far, oldest first; the list is left empty.
  pub fn take_warnings(&self) -> Vec<String> {
    self.warnings.take()
  }
}

/// The two config files for `roots`, the project one found from `cwd`.
pub fn config_files(roots: &Roots, cwd: Option<&Path>) -> ConfigFiles {
  let user_dir = match roots.env().get("TOOLU_USER_CONFIG_DIR") {
    Some(dir) => PathBuf::from(dir),
    None => roots.config_root(),
  };
  ConfigFiles {
    user: user_dir.join("toolu.config.json"),
    project: roots.project_config_path(cwd),
  }
}

/// `[ -f path ]`: a regular file, following symlinks.
pub fn is_file(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
}

/// Whether either config file exists; stat only.
pub fn exists(roots: &Roots, cwd: Option<&Path>) -> bool {
  let files = config_files(roots, cwd);
  is_file(&files.user) || files.project.as_deref().is_some_and(is_file)
}

/// jq `$u * $p`: objects merge recursively; anything else on the project side replaces.
pub fn merge(user: &Value, project: &Value) -> Value {
  match (user, project) {
    (Value::Object(user), Value::Object(project)) => Value::Object(merge_maps(user, project)),
    _ => project.clone(),
  }
}

fn merge_maps(user: &Map<String, Value>, project: &Map<String, Value>) -> Map<String, Value> {
  let mut merged = user.clone();
  for (key, value) in project {
    let value = user
      .get(key)
      .map_or_else(|| value.clone(), |mine| merge(mine, value));
    merged.insert(key.clone(), value);
  }
  merged
}

/// What one file contributed.
enum Layer {
  Absent,
  Malformed,
  Invalid(String),
  Object(Map<String, Value>),
}

/// Malformed JSON, which the loader ignores. Every other [`check_text`] error
/// is an envelope rejection.
const MALFORMED: &str = "malformed JSON";

/// The per-file rule: malformed JSON, a non-object top level, an unknown
/// top-level key, or a `version` other than 1.
///
/// # Errors
/// `malformed JSON` when the text is not usable JSON (`null` and `false`
/// included). Any other error is the envelope reason, with the loader's exact
/// wording.
pub fn check_text(text: &str) -> Result<Map<String, Value>, String> {
  match serde_json::from_str::<Value>(text) {
    Ok(Value::Null | Value::Bool(false)) | Err(_) => Err(MALFORMED.to_owned()),
    Ok(Value::Object(map)) => envelope_error(text, &map).map_or(Ok(map), Err),
    Ok(Value::Bool(true) | Value::Number(_) | Value::String(_) | Value::Array(_)) => {
      Err("top level is not a JSON object".to_owned())
    }
  }
}

/// `jq -e .` over the file: unreadable, unparsable, `null` and `false` are malformed.
fn read_layer(path: &Path) -> Layer {
  if !is_file(path) {
    return Layer::Absent;
  }
  let Ok(bytes) = std::fs::read(path) else {
    return Layer::Malformed;
  };
  let text = String::from_utf8_lossy(&bytes);
  match check_text(&text) {
    Ok(map) => Layer::Object(map),
    Err(reason) if reason == MALFORMED => Layer::Malformed,
    Err(reason) => Layer::Invalid(reason),
  }
}

/// The first envelope rule `map` (parsed from `text`) breaks.
fn envelope_error(text: &str, map: &Map<String, Value>) -> Option<String> {
  let keys = top_level_keys(text).unwrap_or_else(|| map.keys().cloned().collect());
  let unknown: Vec<String> = keys
    .into_iter()
    .filter(|key| !KNOWN_KEYS.contains(&key.as_str()))
    .map(|key| format!("'{key}'"))
    .collect();
  if !unknown.is_empty() {
    let plural = if unknown.len() == 1 { "" } else { "s" };
    return Some(format!(
      "unknown top-level key{plural} {}",
      unknown.join(", ")
    ));
  }
  match map.get("version") {
    Some(version) if version.as_f64() != Some(1.0) => Some(format!(
      "unsupported version {} (supported: 1)",
      stringify(version)
    )),
    Some(_) | None => None,
  }
}

/// Loads and merges the user and project config for `roots`.
pub fn load(roots: &Roots, cwd: Option<&Path>) -> LoadedConfig {
  let files = config_files(roots, cwd);
  let mut warnings = Vec::new();
  let mut invalid = None;
  let mut layers = Vec::new();
  for path in [Some(&files.user), files.project.as_ref()]
    .into_iter()
    .flatten()
  {
    match read_layer(path) {
      Layer::Absent => {}
      Layer::Malformed => warnings.push(format!("malformed JSON in {}; ignoring", path.display())),
      Layer::Invalid(reason) => {
        let text = format!("{}: {reason}", path.display());
        warnings.push(format!("{text}; failing closed (every gate blocks)"));
        invalid = invalid.or(Some(text));
      }
      Layer::Object(map) => layers.push(map),
    }
  }
  let data = match invalid {
    Some(_) => Map::new(),
    None => layers
      .iter()
      .fold(Map::new(), |merged, layer| merge_maps(&merged, layer)),
  };
  LoadedConfig {
    data,
    invalid,
    files,
    host: roots.host(),
    warnings: RefCell::new(warnings),
  }
}

#[cfg(test)]
#[path = "tests/load_test.rs"]
mod tests;
