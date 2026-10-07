//! Classify each profile the way `setup.ts` does, and reject a bad template.

use std::fs;
use std::path::Path;

use super::{Action, MARKER, PROFILES, Profile};

/// Append one `PLAN` line per profile and return the actions in profile order.
pub(super) fn planned(
  stdout: &mut String,
  dir: Option<&Path>,
  agents: &str,
  command: &str,
) -> Vec<Action> {
  PROFILES
    .iter()
    .map(|profile| {
      let target = format!("{agents}/{}.toml", profile.name);
      let action = if command == "remove" {
        classify_remove(&target)
      } else {
        classify_install(dir, profile, &target)
      };
      stdout.push('\n');
      stdout.push_str("PLAN ");
      stdout.push_str(profile.name);
      stdout.push(' ');
      stdout.push_str(action_name(action));
      action
    })
    .collect()
}

/// Whether `text` is one of the five managed profiles.
pub(super) fn valid(text: &str, profile: &Profile) -> bool {
  let lines: Vec<&str> = text.lines().collect();
  let name = format!("name = \"{}\"", profile.name);
  let model = format!("model = \"{}\"", profile.model);
  let effort = format!("model_reasoning_effort = \"{}\"", profile.effort);
  let sandbox = format!("sandbox_mode = \"{}\"", profile.sandbox);
  count(&lines, MARKER) == 1
    && count(&lines, &name) == 1
    && lines.iter().filter(|line| description_line(line)).count() == 1
    && count(&lines, &model) == 1
    && count(&lines, &effort) == 1
    && count(&lines, &sandbox) == 1
    && count(&lines, "developer_instructions = \"\"\"") == 1
    && count(&lines, "\"\"\"") == 1
    && well_formed(&lines)
}

fn action_name(action: Action) -> &'static str {
  match action {
    Action::Install => "install",
    Action::Unchanged => "unchanged",
    Action::Update => "update",
    Action::Conflict => "conflict",
    Action::Absent => "absent",
    Action::Remove => "remove",
  }
}

fn classify_install(dir: Option<&Path>, profile: &Profile, target: &str) -> Action {
  if !Path::new(target).exists() {
    return Action::Install;
  }
  if same_bytes(dir, profile, target) {
    return Action::Unchanged;
  }
  if managed(target) {
    Action::Update
  } else {
    Action::Conflict
  }
}

fn classify_remove(target: &str) -> Action {
  if !Path::new(target).exists() {
    Action::Absent
  } else if managed(target) {
    Action::Remove
  } else {
    Action::Conflict
  }
}

fn same_bytes(dir: Option<&Path>, profile: &Profile, target: &str) -> bool {
  let Ok(current) = fs::read(target) else {
    return false;
  };
  match super::template_text(dir, profile) {
    Ok(text) => current == text.as_bytes(),
    Err(_err) => false,
  }
}

fn managed(path: &str) -> bool {
  fs::read_to_string(path).is_ok_and(|text| text.lines().any(|line| line == MARKER))
}

fn count(lines: &[&str], want: &str) -> usize {
  lines.iter().filter(|line| **line == want).count()
}

fn description_line(line: &str) -> bool {
  let Some(rest) = line.strip_prefix("description = \"") else {
    return false;
  };
  let Some(body) = rest.strip_suffix('"') else {
    return false;
  };
  !body.is_empty() && !body.starts_with('"')
}

fn well_formed(lines: &[&str]) -> bool {
  let mut block = false;
  let mut assignments = 0;
  let mut starts = 0;
  let mut closes = 0;
  for line in lines {
    if line.starts_with('#') || line.trim().is_empty() {
      continue;
    }
    if block {
      if *line == "\"\"\"" {
        block = false;
        closes += 1;
      }
      continue;
    }
    if *line == "developer_instructions = \"\"\"" {
      block = true;
      starts += 1;
    } else if assignment(line) {
      assignments += 1;
    } else {
      return false;
    }
  }
  !block && assignments == 5 && starts == 1 && closes == 1
}

fn assignment(line: &str) -> bool {
  let Some((key, rest)) = line.split_once(" = ") else {
    return false;
  };
  matches!(
    key,
    "name" | "description" | "model" | "model_reasoning_effort" | "sandbox_mode"
  ) && rest.starts_with('"')
    && rest.ends_with('"')
    && rest.len() > 1
}

#[cfg(test)]
#[path = "tests/classify_test.rs"]
mod tests;
