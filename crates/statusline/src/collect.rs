//! Project facts for the line: the injected status snapshot and local profile markers.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use serde_json::Value;
use toolu_engine::status::StatusSnapshot;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_state::git::common_dir_without_git;

use crate::payload::Payload;

/// The local facts the coloured Claude line can display.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Project {
  pub folder: String,
  pub repo_root: String,
  pub branch: String,
  pub ahead: u64,
  pub behind: u64,
  pub staged: u64,
  pub unstaged: u64,
  pub untracked: u64,
  pub gate_failing: bool,
  pub comemory_count: Option<String>,
  pub account_domain: String,
  pub jev: Jev,
}

/// The local readiness of the native Jev command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum Jev {
  #[default]
  Hidden,
  Ready,
  Unavailable(String),
}

fn text<'a>(doc: &'a Value, path: &[&str]) -> &'a str {
  path
    .iter()
    .try_fold(doc, |at, key| at.get(*key))
    .and_then(Value::as_str)
    .unwrap_or("")
}

fn count(doc: &Value, path: &[&str]) -> u64 {
  path
    .iter()
    .try_fold(doc, |at, key| at.get(*key))
    .and_then(Value::as_u64)
    .unwrap_or(0)
}

fn read_json(path: &Path) -> Option<Value> {
  let bytes = std::fs::read(path).ok()?;
  serde_json::from_slice(&bytes).ok()
}

fn folder_name(dir: &Path) -> String {
  dir.file_name().map_or_else(
    || {
      if dir == Path::new("/") {
        "/".to_owned()
      } else {
        String::new()
      }
    },
    |name| name.to_string_lossy().into_owned(),
  )
}

fn snapshot(roots: &Roots, dir: &Path, source: &dyn StatusSnapshot) -> Option<Value> {
  // The bar is optional; keep model and context when status is unreadable.
  source.snapshot_cached(roots, dir).ok()
}

fn account_domain(env: &Env) -> String {
  let home = env
    .get("CLAUDE_CONFIG_DIR")
    .map_or_else(|| env.home(), PathBuf::from);
  let Some(doc) = read_json(&home.join(".claude.json")) else {
    return String::new();
  };
  text(&doc, &["oauthAccount", "emailAddress"])
    .split_once('@')
    .map_or_else(String::new, |(_, domain)| domain.to_owned())
}

fn comemory(roots: &Roots, dir: &Path) -> Option<String> {
  let common = common_dir_without_git(roots.env(), dir)?;
  let main = common.parent()?.file_name()?.to_string_lossy();
  let path = roots
    .config_root()
    .join("comemory-status")
    .join(format!("{main}.json"));
  let doc = read_json(&path)?;
  doc.get("count")?.as_number().map(ToString::to_string)
}

fn local_gate(roots: &Roots, dir: &Path) -> bool {
  let Some(state) = roots.project_state_root(Some(dir), Some(dir)) else {
    return false;
  };
  read_json(&state.join("quality-gate-status.json"))
    .is_some_and(|doc| text(&doc, &["status"]) == "failing")
}

fn executable_toolu(env: &Env) -> bool {
  let Some(path) = env.get("PATH") else {
    return false;
  };
  path.split(':').any(|dir| {
    let found = Path::new(dir).join("toolu");
    std::fs::metadata(found)
      .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
  })
}

fn jev(env: &Env) -> Jev {
  let mut reasons = Vec::new();
  if !executable_toolu(env) {
    reasons.push("missing toolu jev");
  }
  match env.get("TYPESAFE_API_KEY") {
    None | Some("") => reasons.push("missing TYPESAFE_API_KEY"),
    Some(key) if key.contains('\r') || key.contains('\n') => {
      reasons.push("invalid TYPESAFE_API_KEY");
    }
    Some(_) => {}
  }
  if reasons.is_empty() {
    Jev::Ready
  } else {
    Jev::Unavailable(reasons.join("; "))
  }
}

/// Collect status without spawning git for repository discovery or branch lookup.
pub(crate) fn collect(payload: &Payload, roots: &Roots, source: &dyn StatusSnapshot) -> Project {
  let mut found = Project {
    account_domain: account_domain(roots.env()),
    jev: jev(roots.env()),
    ..Project::default()
  };
  if payload.cwd.is_empty() {
    return found;
  }
  let dir = Path::new(&payload.cwd);
  if !dir.is_dir() {
    return found;
  }
  found.folder = folder_name(dir);
  let Some(doc) = snapshot(roots, dir, source) else {
    return found;
  };
  text(&doc, &["repo_root"]).clone_into(&mut found.repo_root);
  let branch = text(&doc, &["branch"]);
  found.branch = if branch == "HEAD" {
    String::new()
  } else {
    branch.to_owned()
  };
  found.ahead = count(&doc, &["ahead"]);
  found.behind = count(&doc, &["behind"]);
  found.staged = count(&doc, &["working_tree", "staged"]);
  found.unstaged = count(&doc, &["working_tree", "unstaged"]);
  found.untracked = count(&doc, &["working_tree", "untracked"]);
  let gate = text(&doc, &["gate", "status"]);
  found.gate_failing = gate == "failing"
    || (matches!(gate, "" | "missing") && found.repo_root.is_empty() && local_gate(roots, dir));
  if !found.repo_root.is_empty() {
    found.comemory_count = comemory(roots, dir);
  }
  found
}

#[cfg(test)]
#[path = "tests/collect_test.rs"]
mod tests;
