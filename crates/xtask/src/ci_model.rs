//! `.github/ci-paths.json`: groups, globs, and diff classification (#458).

use std::path::Path;

use regex::Regex;
use serde_json::{Map, Value};

use crate::ci_glob::{self, Glob};

/// The data file, relative to the repository root.
pub(crate) const CI_PATHS_FILE: &str = ".github/ci-paths.json";

/// The synthetic group that is on when any non-release file changed.
pub(crate) const CHANGED: &str = "changed";

const CHANGELOG: &str = "CHANGELOG.md";

/// A gated workflow in the data file.
#[derive(Debug, Clone)]
pub(crate) struct WorkflowSpec {
  /// The aggregate job, when this workflow has one.
  pub(crate) aggregate: Option<String>,
  /// Job names branch protection requires besides the aggregate.
  pub(crate) required: Vec<String>,
  /// Gated job id and its group, in file order.
  pub(crate) jobs: Vec<(String, String)>,
}

/// The parsed path-group file.
pub(crate) struct CiPaths {
  groups: Vec<(String, Vec<Glob>)>,
  run_everything: Vec<Glob>,
  release_paths: Vec<Glob>,
  /// Workflow file name and its spec, in file order.
  pub(crate) workflows: Vec<(String, WorkflowSpec)>,
  version_line: Regex,
}

/// One changed path and the `+`/`-` lines of its diff.
pub(crate) struct ChangedFile {
  /// Repository-relative path.
  pub(crate) path: String,
  /// Added and removed lines, empty when the path is not release-only.
  pub(crate) lines: Vec<String>,
}

/// Group switches plus the log lines that explain them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Classification {
  /// Output name and whether that group is on, `changed` last.
  pub(crate) outputs: Vec<(String, bool)>,
  /// Why the outputs are what they are.
  pub(crate) reasons: Vec<String>,
}

/// Load the data file of the repository at `root`.
pub(crate) fn load_repo(root: &Path) -> Result<CiPaths, String> {
  let path = root.join(CI_PATHS_FILE);
  let text = std::fs::read_to_string(&path)
    .map_err(|err| format!("{} is not readable JSON: {err}", path.display()))?;
  let value: Value = serde_json::from_str(&text)
    .map_err(|err| format!("{} is not readable JSON: {err}", path.display()))?;
  parse_config(&path.display().to_string(), &value)
}

/// Every output on, with `reason` logged.
pub(crate) fn all_on(config: &CiPaths, reason: &str) -> Classification {
  Classification {
    outputs: output_names(config)
      .into_iter()
      .map(|name| (name, true))
      .collect(),
    reasons: vec![format!("every group on: {reason}")],
  }
}

/// Classify a diff. An empty diff turns everything on.
pub(crate) fn classify(config: &CiPaths, files: &[ChangedFile]) -> Classification {
  if files.is_empty() {
    return all_on(config, "the diff is empty");
  }
  let mut outputs: Vec<(String, bool)> = output_names(config)
    .into_iter()
    .map(|name| (name, false))
    .collect();
  let mut reasons = Vec::new();
  for file in files {
    if is_release_only(config, file) {
      reasons.push(format!("{}: release-only", file.path));
      continue;
    }
    set_output(&mut outputs, CHANGED, true);
    if ci_glob::matches(&config.run_everything, &file.path) {
      return all_on(config, &format!("{} runs everything", file.path));
    }
    let groups = matching_groups(config, &file.path);
    if groups.is_empty() {
      return all_on(config, &format!("{} matches no group", file.path));
    }
    for name in &groups {
      set_output(&mut outputs, name, true);
    }
    reasons.push(format!("{}: {}", file.path, groups.join(", ")));
  }
  Classification { outputs, reasons }
}

fn output_names(config: &CiPaths) -> Vec<String> {
  let mut names: Vec<String> = config.groups.iter().map(|(name, _)| name.clone()).collect();
  names.push(CHANGED.to_owned());
  names
}

fn set_output(outputs: &mut [(String, bool)], name: &str, on: bool) {
  if let Some(slot) = outputs.iter_mut().find(|(key, _)| key == name) {
    slot.1 = on;
  }
}

fn matching_groups(config: &CiPaths, path: &str) -> Vec<String> {
  config
    .groups
    .iter()
    .filter(|(_, globs)| ci_glob::matches(globs, path))
    .map(|(name, _)| name.clone())
    .collect()
}

impl CiPaths {
  /// Whether `path` is one of the release-only paths.
  pub(crate) fn matches_release(&self, path: &str) -> bool {
    ci_glob::matches(&self.release_paths, path)
  }

  /// Whether `name` is a group in the data file.
  pub(crate) fn group_defined(&self, name: &str) -> bool {
    self.groups.iter().any(|(group, _)| group == name)
  }

  /// Every glob string, duplicates removed.
  pub(crate) fn glob_sources(&self) -> Vec<String> {
    let mut sources = Vec::new();
    for (_, globs) in &self.groups {
      sources.extend(globs.iter().map(|glob| glob.source.clone()));
    }
    sources.extend(self.run_everything.iter().map(|glob| glob.source.clone()));
    sources.extend(self.release_paths.iter().map(|glob| glob.source.clone()));
    sources.sort();
    sources.dedup();
    sources
  }

  /// Whether `path` matches the glob `source`.
  pub(crate) fn matches_glob(&self, source: &str, path: &str) -> bool {
    self
      .groups
      .iter()
      .flat_map(|(_, globs)| globs)
      .chain(self.run_everything.iter())
      .chain(self.release_paths.iter())
      .any(|glob| glob.source == source && glob.hits(path))
  }
}

fn is_release_only(config: &CiPaths, file: &ChangedFile) -> bool {
  if !ci_glob::matches(&config.release_paths, &file.path) {
    return false;
  }
  if file.path == CHANGELOG {
    return true;
  }
  !file.lines.is_empty()
    && file
      .lines
      .iter()
      .all(|line| config.version_line.is_match(line))
}

fn parse_config(path: &str, value: &Value) -> Result<CiPaths, String> {
  let obj = object(Some(value), path)?;
  require_keys(
    path,
    obj,
    &["groups", "runEverything", "releaseOnly", "workflows"],
  )?;
  let groups = parse_groups(path, obj.get("groups"))?;
  let run_everything = globs(path, obj.get("runEverything"), "runEverything")?;
  let release = object(obj.get("releaseOnly"), &format!("{path}.releaseOnly"))?;
  require_keys(
    &format!("{path}.releaseOnly"),
    release,
    &["paths", "versionKeys"],
  )?;
  let release_paths = globs(path, release.get("paths"), "releaseOnly.paths")?;
  let version_keys = string_list(
    release.get("versionKeys"),
    &format!("{path}.releaseOnly.versionKeys"),
  )?;
  if version_keys.is_empty() {
    return Err(format!("{path}.releaseOnly.versionKeys is empty"));
  }
  let version_line = ci_glob::version_pattern(&version_keys)?;
  let workflows = parse_workflows(path, obj.get("workflows"))?;
  Ok(CiPaths {
    groups,
    run_everything,
    release_paths,
    workflows,
    version_line,
  })
}

fn parse_groups(path: &str, value: Option<&Value>) -> Result<Vec<(String, Vec<Glob>)>, String> {
  let obj = object(value, &format!("{path}.groups"))?;
  let mut groups = Vec::new();
  for (name, globs_value) in obj {
    if !group_name(name) {
      return Err(format!("{path}.groups.{name} is not a group name"));
    }
    groups.push((
      name.clone(),
      globs(path, Some(globs_value), &format!("groups.{name}"))?,
    ));
  }
  if groups.is_empty() {
    return Err(format!("{path}.groups is empty"));
  }
  Ok(groups)
}

fn parse_workflows(
  path: &str,
  value: Option<&Value>,
) -> Result<Vec<(String, WorkflowSpec)>, String> {
  let obj = object(value, &format!("{path}.workflows"))?;
  let mut workflows = Vec::new();
  for (file, spec) in obj {
    if !yaml_file(file) {
      return Err(format!(
        "{path}.workflows.{file} is not a workflow file name"
      ));
    }
    workflows.push((
      file.clone(),
      parse_workflow(&format!("{path}.workflows.{file}"), spec)?,
    ));
  }
  Ok(workflows)
}

fn parse_workflow(path: &str, value: &Value) -> Result<WorkflowSpec, String> {
  let obj = object(Some(value), path)?;
  require_keys(path, obj, &["aggregate", "required", "jobs"])?;
  let aggregate = match obj.get("aggregate") {
    Some(Value::Null) => None,
    Some(Value::String(name)) if !name.is_empty() => Some(name.clone()),
    _ => return Err(format!("{path}.aggregate is not a job name or null")),
  };
  let required = string_list(obj.get("required"), &format!("{path}.required"))?;
  let jobs_obj = object(obj.get("jobs"), &format!("{path}.jobs"))?;
  let mut jobs = Vec::new();
  for (id, group) in jobs_obj {
    let Some(group) = group.as_str().filter(|group| !group.is_empty()) else {
      return Err(format!("{path}.jobs.{id} is not a group name"));
    };
    jobs.push((id.clone(), group.to_owned()));
  }
  Ok(WorkflowSpec {
    aggregate,
    required,
    jobs,
  })
}

fn globs(path: &str, value: Option<&Value>, label: &str) -> Result<Vec<Glob>, String> {
  let items = string_list(value, &format!("{path}.{label}"))?;
  if items.is_empty() {
    return Err(format!("{path}.{label} is empty"));
  }
  items
    .into_iter()
    .map(|source| ci_glob::compile(path, source))
    .collect()
}

fn yaml_file(name: &str) -> bool {
  std::path::Path::new(name)
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
}

fn group_name(name: &str) -> bool {
  let mut chars = name.chars();
  chars.next().is_some_and(|ch| ch.is_ascii_lowercase())
    && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn require_keys(path: &str, obj: &Map<String, Value>, keys: &[&str]) -> Result<(), String> {
  for key in obj.keys() {
    if !keys.contains(&key.as_str()) {
      return Err(format!("{path}: unknown key {key}"));
    }
  }
  for key in keys {
    if !obj.contains_key(*key) {
      return Err(format!("{path}: missing {key}"));
    }
  }
  Ok(())
}

fn object<'a>(value: Option<&'a Value>, path: &str) -> Result<&'a Map<String, Value>, String> {
  value
    .and_then(Value::as_object)
    .ok_or_else(|| format!("{path} is not an object"))
}

fn string_list(value: Option<&Value>, path: &str) -> Result<Vec<String>, String> {
  let Some(Value::Array(items)) = value else {
    return Err(format!("{path} is not an array"));
  };
  items
    .iter()
    .map(|item| {
      item
        .as_str()
        .filter(|text| !text.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| format!("{path} has an empty entry"))
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/ci_model_test.rs"]
mod tests;
