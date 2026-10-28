use super::reanchor;
use crate::contract::Side;
use crate::memory::Memory;
use crate::review::mask::Mask;
use crate::shape::Shaped;
use crate::validate::anchors;
use crate::validate::tests::{file, line, model, run, shaped};

const GUARD: &str = "    let count = stats.writes.len() as u64;";
const DIVIDE: &str = "    total / count";

/// One file whose changed new-side lines are `texts`, numbered from 10.
fn diff(texts: &[&str]) -> Shaped {
  let lines = (10..)
    .zip(texts)
    .map(|(number, text)| line(Side::New, number, text))
    .collect();
  shaped(vec![file("src/m.rs", lines)], &["src/m.rs"])
}

fn moved(shaped: &Shaped, line_no: u32, quote: &str) -> Option<(u32, u32)> {
  let mask = Mask::new(shaped, &[]).unwrap();
  let anchors = anchors(shaped);
  reanchor(&model("src/m.rs", line_no, quote), &anchors, &mask)
    .map(|found| (found.line, found.end_line))
}

#[test]
fn a_misnumbered_quote_moves_to_the_one_line_that_holds_it() {
  let shaped = diff(&["fn mean() -> u64 {", GUARD, DIVIDE]);
  assert_eq!(moved(&shaped, 14, GUARD), Some((11, 11)));
  // Whitespace runs compare collapsed, as grounding compares them.
  assert_eq!(
    moved(&shaped, 3, "let count =   stats.writes.len() as u64;"),
    Some((11, 11))
  );
}

#[test]
fn a_multi_line_quote_moves_to_its_consecutive_run() {
  let shaped = diff(&["fn mean() -> u64 {", GUARD, DIVIDE]);
  let quote = format!("{GUARD}\n{DIVIDE}");
  assert_eq!(moved(&shaped, 2, &quote), Some((11, 12)));
}

#[test]
fn an_ambiguous_short_missing_or_other_side_quote_stays_put() {
  let twice = diff(&[GUARD, DIVIDE, GUARD]);
  assert_eq!(moved(&twice, 30, GUARD), None);
  let short = diff(&["}", "Ok(())", "}"]);
  assert_eq!(moved(&short, 30, "Ok(())"), None);
  let once = diff(&[GUARD]);
  assert_eq!(
    moved(&once, 30, "let count = stats.reads.len() as u64;"),
    None
  );
  // A removed line never anchors a new-side finding.
  let removed = shaped(
    vec![file("src/m.rs", vec![line(Side::Old, 10, GUARD)])],
    &["src/m.rs"],
  );
  assert_eq!(moved(&removed, 30, GUARD), None);
}

#[test]
fn validate_keeps_a_reanchored_finding_and_says_where_it_moved() {
  let shaped = diff(&["fn mean() -> u64 {", GUARD, DIVIDE]);
  let found = [model("src/m.rs", 14, DIVIDE)];
  let out = run((&shaped, &found, &[], &Memory::default(), &[], &[]));
  assert_eq!(out.findings.len(), 1);
  assert_eq!((out.findings[0].line, out.findings[0].end_line), (12, 12));
  assert_eq!(out.findings[0].quote, DIVIDE);
  assert!(out.unreviewed.is_empty(), "{:?}", out.unreviewed);
  assert_eq!(out.dropped, 0);
  assert_eq!(
    out.notes,
    ["src/m.rs:14..14 (new) re-anchored to 12..12, where its quote is"]
  );
}

#[test]
fn validate_still_drops_an_ambiguous_quote_and_loses_its_coverage() {
  let shaped = diff(&[GUARD, DIVIDE, GUARD]);
  let found = [model("src/m.rs", 30, GUARD)];
  let out = run((&shaped, &found, &[], &Memory::default(), &[], &[]));
  assert_eq!(out.findings.len(), 0);
  assert_eq!(out.dropped, 1);
  assert_eq!(out.unreviewed.len(), 1);
  assert_eq!(out.unreviewed[0].path, "src/m.rs");
  assert_eq!(out.notes.len(), 0);
}
