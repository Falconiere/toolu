use super::render_table;
use crate::support::graph_248;

#[test]
fn the_table_lists_status_order_batch_and_missing_checkouts() {
  let table = render_table(&graph_248());
  let lines: Vec<&str> = table.lines().collect();
  assert_eq!(
    lines[0],
    "Epic Falconiere/comemory#248 [open] [Realtime replication] Shared data everywhere with a required resident daemon"
  );
  assert_eq!(
    lines[1],
    "counts: done=5, in_flight=0, ready=3, blocked=6, external_blocked=0"
  );
  assert_eq!(
    lines[2],
    format!(
      "{:<34} {:<17} wave chain unblocks  open blockers",
      "issue", "status"
    )
  );
  assert!(lines[3].starts_with("CodaSignal/comemory.io#183"));
  assert!(lines[3].contains(" ready "));
  assert!(table.contains("launch batch (max 3, 0 in flight): Falconiere/comemory#255, CodaSignal/comemory.io#183, Falconiere/comemory#254"));
  assert!(
    table
      .ends_with("no local checkout (clone into /Volumes/Projects/ccvps): Falconiere/homebrew-tap")
  );
}

#[test]
fn a_cycle_and_a_complete_epic_get_their_own_lines() {
  let mut graph = graph_248();
  graph.cycle = vec!["a#1".into(), "a#2".into()];
  graph.missing_checkouts.clear();
  graph.complete = true;
  graph.launch_batch.clear();
  let table = render_table(&graph);
  assert!(table.contains("launch batch (max 3, 0 in flight): none"));
  assert!(table.contains("CYCLE (cannot schedule): a#1, a#2"));
  assert!(table.ends_with("ALL SUB-ISSUES CLOSED - epic complete"));
}
