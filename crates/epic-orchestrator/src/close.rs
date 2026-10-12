//! Close a completed epic in its tracker (`epic-close.ts`): post the
//! sub-issue to merged-PR summary, then close (GitHub), move to Done (Jira) or
//! complete (Linear). Refuses while any sub-issue is open.

use std::path::Path;

use serde_json::{Value, json};

use crate::graph::Graph;
use crate::tracker::Tracker;

/// The summary posted on the epic: one row per sub-issue with its merged PRs,
/// admin merges noted.
pub(crate) fn summary_table(graph: &Graph) -> String {
  let state = Path::new(&graph.state_dir);
  let mut lines = vec![
    "Epic complete.".to_owned(),
    String::new(),
    "| Sub-issue | Merged PR |".to_owned(),
    "|---|---|".to_owned(),
  ];
  for issue in &graph.issues {
    let record = crate::disk::read_value(&state.join("issues").join(format!("{}.json", issue.key)))
      .unwrap_or(Value::Null);
    let admin = if record.pointer("/last_gate/admin_used") == Some(&Value::Bool(true)) {
      " (admin merge)"
    } else {
      ""
    };
    let merged: Vec<&str> = issue
      .prs
      .iter()
      .filter(|pr| pr.state == "MERGED")
      .map(|pr| pr.url.as_str())
      .collect();
    let prs = if merged.is_empty() {
      "closed without a merged PR".to_owned()
    } else {
      merged.join(", ")
    };
    lines.push(format!(
      "| {} — {} | {prs}{admin} |",
      issue.reference,
      issue.title.replace('|', "\\|")
    ));
  }
  lines.join("\n")
}

/// Refuse an incomplete epic with its open references.
///
/// # Errors
/// A sub-issue is still open.
pub(crate) fn require_complete(graph: &Graph) -> Result<(), String> {
  if graph.complete {
    return Ok(());
  }
  let open: Vec<&str> = graph
    .issues
    .iter()
    .filter(|issue| issue.state != "closed")
    .map(|issue| issue.reference.as_str())
    .collect();
  Err(format!(
    "epic not complete; still open: {}. Re-run the graph first.",
    open.join(", ")
  ))
}

/// Post the summary and close the epic in `tracker`.
///
/// # Errors
/// The epic is incomplete, or the tracker fails.
pub(crate) fn close_epic(graph: &Graph, tracker: &dyn Tracker) -> Result<Value, String> {
  require_complete(graph)?;
  let result = tracker.close_epic(&summary_table(graph))?;
  Ok(json!({"epic": graph.epic.reference, "tracker": tracker.kind().name(), "result": result}))
}

#[cfg(test)]
#[path = "tests/close_test.rs"]
mod tests;
