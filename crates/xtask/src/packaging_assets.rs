//! Skill, agent, and hook checks for `cargo xtask packaging`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use regex::Regex;
use serde_json::Value;

pub(crate) const PLUGINS: usize = 12;
pub(crate) const SKILLS: usize = 14;
pub(crate) const AGENTS: usize = 5;
pub(crate) const HOOKS: usize = 10;

const REQUIRED_SKILLS: [&str; 5] = [
  "plugins/toolu/skills/commit/SKILL.md",
  "plugins/toolu/skills/review-and-commit/SKILL.md",
  "plugins/toolu/skills/setup/SKILL.md",
  "plugins/statusline/skills/status/SKILL.md",
  "plugins/pr-babysit/skills/babysit/SKILL.md",
];

const EFFORTS: [&str; 6] = ["low", "medium", "high", "xhigh", "max", "ultra"];
const SANDBOXES: [&str; 2] = ["read-only", "workspace-write"];

/// Discoverable skills, including the required Codex equivalents.
pub(crate) fn check_skills(root: &Path) -> Result<usize, String> {
  let mut files = Vec::new();
  for plugin in dirs(root, "plugins")? {
    for skill in dirs(root, &format!("{plugin}/skills"))? {
      let file = format!("{skill}/SKILL.md");
      if root.join(&file).is_file() {
        files.push(file);
      }
    }
  }
  for file in &files {
    check_skill(root, file)?;
  }
  if files.len() != SKILLS {
    return Err(format!(
      "expected {SKILLS} discoverable skills, found {}",
      files.len()
    ));
  }
  for skill in REQUIRED_SKILLS {
    if !root.join(skill).is_file() {
      return Err(format!("missing Codex command-equivalent skill: {skill}"));
    }
  }
  Ok(files.len())
}

fn check_skill(root: &Path, file: &str) -> Result<(), String> {
  let text =
    std::fs::read_to_string(root.join(file)).map_err(|err| format!("cannot read {file}: {err}"))?;
  let lines: Vec<&str> = text.lines().collect();
  if lines.first().copied() != Some("---") {
    return Err(format!("{file} is missing opening frontmatter"));
  }
  let name = front_value(lines.get(1).copied().unwrap_or(""), "name");
  let description = front_value(lines.get(2).copied().unwrap_or(""), "description");
  if name.is_empty() {
    return Err(format!("{file} is missing a frontmatter name"));
  }
  if description.is_empty() {
    return Err(format!("{file} is missing a frontmatter description"));
  }
  if !skill_name(&name) {
    return Err(format!("{file} has an invalid skill name"));
  }
  if name != parent_base(file) {
    return Err(format!("{file} name differs from its directory"));
  }
  if !lines.iter().skip(1).any(|line| *line == "---") {
    return Err(format!("{file} is missing closing frontmatter"));
  }
  Ok(())
}

/// Codex agent profiles under `plugins/toolu/assets/agents`.
pub(crate) fn check_agents(root: &Path) -> Result<usize, String> {
  let dir = root.join("plugins/toolu/assets/agents");
  let mut files = Vec::new();
  if dir.is_dir() {
    for entry in std::fs::read_dir(&dir).map_err(|err| format!("cannot list agents: {err}"))? {
      let entry = entry.map_err(|err| format!("cannot list agents: {err}"))?;
      let name = entry.file_name().to_string_lossy().into_owned();
      if Path::new(&name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
      {
        files.push(format!("plugins/toolu/assets/agents/{name}"));
      }
    }
  }
  files.sort();
  for file in &files {
    check_agent(root, file)?;
  }
  if files.len() != AGENTS {
    return Err(format!(
      "expected {AGENTS} Codex agent profiles, found {}",
      files.len()
    ));
  }
  Ok(files.len())
}

fn check_agent(root: &Path, file: &str) -> Result<(), String> {
  let text = std::fs::read_to_string(root.join(file))
    .map_err(|err| format!("invalid agent TOML: {file}: {err}"))?;
  let doc: toml::Value =
    toml::from_str(&text).map_err(|err| format!("invalid agent TOML: {file}: {err}"))?;
  let effort = doc
    .get("model_reasoning_effort")
    .and_then(toml::Value::as_str);
  let sandbox = doc.get("sandbox_mode").and_then(toml::Value::as_str);
  let fields = [
    "name",
    "description",
    "model",
    "model_reasoning_effort",
    "sandbox_mode",
    "developer_instructions",
  ];
  let filled = fields.iter().all(|key| filled_text(doc.get(*key)));
  if !filled
    || !EFFORTS.contains(&effort.unwrap_or(""))
    || !SANDBOXES.contains(&sandbox.unwrap_or(""))
  {
    return Err(format!("invalid agent TOML: {file}"));
  }
  let stem = Path::new(file)
    .file_stem()
    .and_then(|stem| stem.to_str())
    .unwrap_or(file);
  if doc.get("name").and_then(toml::Value::as_str) != Some(stem) {
    return Err(format!("{file} name differs from its filename"));
  }
  Ok(())
}

/// Hook manifests whose non-launcher commands are plugin-relative executables.
pub(crate) fn check_hooks(root: &Path) -> Result<usize, String> {
  let mut files = Vec::new();
  for plugin in dirs(root, "plugins")? {
    let file = format!("{plugin}/hooks/hooks.json");
    if root.join(&file).is_file() {
      files.push(file);
    }
  }
  for file in &files {
    check_hook_file(root, file)?;
  }
  if files.len() != HOOKS {
    return Err(format!(
      "expected {HOOKS} hook manifests, found {}",
      files.len()
    ));
  }
  Ok(files.len())
}

fn check_hook_file(root: &Path, file: &str) -> Result<(), String> {
  let doc = read_json(root, file)?;
  let Some(commands) = hook_commands(&doc) else {
    return Err(format!("invalid hook schema: {file}"));
  };
  let plugin_root = file.strip_suffix("/hooks/hooks.json").unwrap_or(file);
  for hook in commands {
    check_hook_command(root, file, plugin_root, hook)?;
  }
  Ok(())
}

fn check_hook_command(
  root: &Path,
  file: &str,
  plugin_root: &str,
  hook: &Value,
) -> Result<(), String> {
  let Some(raw) = hook.get("command").and_then(Value::as_str) else {
    return Ok(());
  };
  if !command_windows_absent(hook) || is_launcher(raw) {
    return Ok(());
  }
  let command = unquote(raw);
  let Some(prefix) = ["${CLAUDE_PLUGIN_ROOT}/", "${PLUGIN_ROOT}/"]
    .iter()
    .find(|prefix| command.starts_with(**prefix))
  else {
    return Err(format!(
      "{file} has a non-plugin-relative hook command: {command}"
    ));
  };
  let hook_path = command
    .get(prefix.len()..)
    .ok_or_else(|| format!("{file} has a non-plugin-relative hook command: {command}"))?;
  let abs = root.join(plugin_root).join(hook_path);
  if !abs.is_file() || !executable(&abs) {
    return Err(format!(
      "{file} references a missing or non-executable hook: {hook_path}"
    ));
  }
  Ok(())
}

fn hook_commands(doc: &Value) -> Option<Vec<&Value>> {
  let hooks = doc.get("hooks")?.as_object()?;
  let mut out = Vec::new();
  for value in hooks.values() {
    for group in value.as_array()? {
      collect_hooks(group, &mut out)?;
    }
  }
  Some(out)
}

fn collect_hooks<'a>(group: &'a Value, out: &mut Vec<&'a Value>) -> Option<()> {
  for hook in group.get("hooks")?.as_array()? {
    if hook.get("type")?.as_str() != Some("command") || !hook.get("command")?.is_string() {
      return None;
    }
    out.push(hook);
  }
  Some(())
}

pub(crate) fn dirs(root: &Path, rel: &str) -> Result<Vec<String>, String> {
  let dir = root.join(rel);
  if !dir.is_dir() {
    return Ok(Vec::new());
  }
  let mut names = Vec::new();
  for entry in std::fs::read_dir(&dir).map_err(|err| format!("cannot list {rel}: {err}"))? {
    let entry = entry.map_err(|err| format!("cannot list {rel}: {err}"))?;
    let name = entry.file_name().to_string_lossy().into_owned();
    if name.starts_with('.') || !entry.path().is_dir() {
      continue;
    }
    names.push(format!("{rel}/{name}"));
  }
  names.sort();
  Ok(names)
}

pub(crate) fn read_json(root: &Path, rel: &str) -> Result<Value, String> {
  let path = root.join(rel);
  if !path.is_file() {
    return Err(format!("missing {rel}"));
  }
  let text = std::fs::read_to_string(&path).map_err(|err| format!("invalid JSON: {rel}: {err}"))?;
  serde_json::from_str(&text).map_err(|err| format!("invalid JSON: {rel}: {err}"))
}

fn front_value(line: &str, key: &str) -> String {
  let prefix = format!("{key}:");
  line
    .strip_prefix(&prefix)
    .map(|rest| rest.trim().to_owned())
    .unwrap_or_default()
}

fn skill_name(name: &str) -> bool {
  !name.is_empty()
    && name
      .bytes()
      .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn parent_base(file: &str) -> &str {
  let parent = file.rsplit_once('/').map_or(file, |(dir, _)| dir);
  parent.rsplit_once('/').map_or(parent, |(_, name)| name)
}

fn filled_text(value: Option<&toml::Value>) -> bool {
  value
    .and_then(toml::Value::as_str)
    .is_some_and(|text| !text.trim().is_empty())
}

fn command_windows_absent(hook: &Value) -> bool {
  match hook.get("commandWindows") {
    None | Some(Value::Null) => true,
    Some(_) => false,
  }
}

fn is_launcher(command: &str) -> bool {
  Regex::new(r"hooks/dist/|\bbun\b").is_ok_and(|expr| expr.is_match(command))
}

fn unquote(raw: &str) -> &str {
  if raw.len() > 1 && raw.starts_with('"') && raw.ends_with('"') {
    return raw.get(1..raw.len() - 1).unwrap_or(raw);
  }
  raw
}

fn executable(path: &Path) -> bool {
  std::fs::metadata(path).is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
#[path = "tests/packaging_assets_test.rs"]
mod tests;
