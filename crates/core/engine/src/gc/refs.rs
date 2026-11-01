//! The `refs/epic-wip/<key>` snapshots `toolu epic finish` takes before it
//! removes a worktree. One goes once every record of its key is merged or
//! abandoned with no leftover files; a key without a record is kept.

use std::path::Path;

use serde_json::{Value, json};

use super::repos::IssueRecord;
use super::{Request, git};

const PREFIX: &str = "refs/epic-wip/";

/// Why a snapshot stays, or `None` when it may go.
pub(crate) fn keep_reason(key: &str, records: &[IssueRecord]) -> Option<String> {
  let mut matched = records.iter().filter(|record| record.key == key).peekable();
  matched.peek()?;
  for record in matched {
    if !record.terminal() {
      return Some(format!("the issue is {}", record.stage));
    }
    if record.leftover_files > 0 {
      return Some(format!(
        "the snapshot holds {} leftover files",
        record.leftover_files
      ));
    }
  }
  None
}

/// The snapshots of `root`: `removed` refs and `kept` refs with reasons.
pub(crate) fn sweep(request: &Request, root: &Path, records: &[IssueRecord]) -> Value {
  let env = &request.env;
  let listed = git(env, root, &["for-each-ref", "--format=%(refname)", PREFIX]);
  let text = match listed {
    Ok(text) => text,
    Err(err) => return json!({"skipped": err}),
  };
  let mut removed = Vec::new();
  let mut kept = Vec::new();
  for refname in text.lines().filter(|line| !line.is_empty()) {
    let key = refname.strip_prefix(PREFIX).unwrap_or(refname);
    let reason = match keep_reason(key, records) {
      Some(reason) => Some(reason),
      None if records.iter().any(|record| record.key == key) => None,
      None => Some("no issue record names it".to_owned()),
    };
    if let Some(reason) = reason {
      kept.push(json!({"ref": refname, "reason": reason}));
      continue;
    }
    if !request.apply {
      removed.push(refname.to_owned());
      continue;
    }
    match git(env, root, &["update-ref", "-d", refname]) {
      Ok(_) => removed.push(refname.to_owned()),
      Err(err) => kept.push(json!({"ref": refname, "reason": err})),
    }
  }
  json!({"removed": removed, "kept": kept})
}

#[cfg(test)]
#[path = "tests/refs_test.rs"]
mod tests;
