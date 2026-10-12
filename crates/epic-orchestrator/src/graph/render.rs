//! The graph table the CLI prints (`renderTable`).

use super::{Graph, GraphIssue};

fn order(status: &str) -> u8 {
  match status {
    "in_flight" => 0,
    "ready" => 1,
    "blocked" => 2,
    "external_blocked" => 3,
    "done" => 4,
    _ => 99,
  }
}

fn row(issue: &GraphIssue) -> String {
  let done = issue.status == "done";
  let unblocks = if done {
    "-".to_owned()
  } else {
    issue.unblocks.to_string()
  };
  let chain = if done {
    "-".to_owned()
  } else {
    issue.chain.to_string()
  };
  let wave = issue
    .wave
    .map_or_else(|| "-".to_owned(), |wave| wave.to_string());
  let blockers = if issue.open_blockers.is_empty() {
    "-".to_owned()
  } else {
    issue.open_blockers.join(", ")
  };
  format!(
    "{:<34} {:<17} {wave:>4} {chain:>5} {unblocks:>8}  {blockers}",
    issue.reference, issue.status
  )
}

/// The table: header, counts, one row per issue (by status, wave, reference),
/// the batch, and any cycle, missing checkout or completion line.
pub(crate) fn render_table(graph: &Graph) -> String {
  let counts = &graph.counts;
  let mut lines = vec![
    format!(
      "Epic {} [{}] {}",
      graph.epic.reference, graph.epic.state, graph.epic.title
    ),
    format!(
      "counts: done={}, in_flight={}, ready={}, blocked={}, external_blocked={}",
      counts.done, counts.in_flight, counts.ready, counts.blocked, counts.external_blocked
    ),
    format!(
      "{:<34} {:<17} {:>4} {:>5} {:>8}  open blockers",
      "issue", "status", "wave", "chain", "unblocks"
    ),
  ];
  let mut sorted: Vec<&GraphIssue> = graph.issues.iter().collect();
  sorted.sort_by(|left, right| {
    order(&left.status)
      .cmp(&order(&right.status))
      .then(left.wave.unwrap_or(99).cmp(&right.wave.unwrap_or(99)))
      .then(left.reference.cmp(&right.reference))
  });
  lines.extend(sorted.into_iter().map(row));
  let batch = if graph.launch_batch.is_empty() {
    "none".to_owned()
  } else {
    graph.launch_batch.join(", ")
  };
  lines.push(format!(
    "launch batch (max {}, {} in flight): {batch}",
    graph.max_parallel,
    graph.in_flight.len()
  ));
  lines.extend(footer(graph));
  lines.join("\n")
}

/// The cycle, missing checkout and completion lines that apply.
fn footer(graph: &Graph) -> Vec<String> {
  let mut lines = Vec::new();
  if !graph.cycle.is_empty() {
    lines.push(format!(
      "CYCLE (cannot schedule): {}",
      graph.cycle.join(", ")
    ));
  }
  if !graph.missing_checkouts.is_empty() {
    lines.push(format!(
      "no local checkout (clone into {}): {}",
      graph.clone_root,
      graph.missing_checkouts.join(", ")
    ));
  }
  if graph.complete {
    lines.push("ALL SUB-ISSUES CLOSED - epic complete".to_owned());
  }
  lines
}

#[cfg(test)]
#[path = "tests/render_test.rs"]
mod tests;
