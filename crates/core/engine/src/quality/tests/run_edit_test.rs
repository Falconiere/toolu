use toolu_state::edit_records::{EditOperation, EditRecord};

use super::{last_edit_records, last_edit_records_probed};

fn record(
  path: &str,
  operation: EditOperation,
  from: Option<&str>,
  moved_to: Option<&str>,
) -> EditRecord {
  EditRecord {
    path: path.to_owned(),
    operation,
    from: from.map(str::to_owned),
    moved_to: moved_to.map(str::to_owned),
  }
}

fn meta(record: &EditRecord) -> (&str, EditOperation, Option<&str>, Option<&str>) {
  (
    record.path.as_str(),
    record.operation,
    record.from.as_deref(),
    record.moved_to.as_deref(),
  )
}

#[test]
fn last_edit_records_mixed_ops_keep_last_occurrence_order_and_metadata() {
  let records = [
    record("A", EditOperation::Write, None, None),
    record("B", EditOperation::Write, None, None),
    record("A", EditOperation::Delete, None, None),
    record("C", EditOperation::Move, Some("c-src"), None),
    record("B", EditOperation::Update, None, None),
  ];
  let survivors = last_edit_records(&records);
  assert_eq!(
    survivors.iter().map(|row| meta(row)).collect::<Vec<_>>(),
    vec![
      ("A", EditOperation::Delete, None, None),
      ("C", EditOperation::Move, Some("c-src"), None),
      ("B", EditOperation::Update, None, None),
    ]
  );
}

#[test]
fn last_edit_records_empty_distinct_identical_and_path_aliases() {
  assert_eq!(last_edit_records(&[]).len(), 0);

  let distinct = [
    record("a", EditOperation::Write, None, None),
    record("b", EditOperation::Delete, None, None),
    record("c", EditOperation::Move, Some("from-c"), None),
  ];
  assert_eq!(
    last_edit_records(&distinct)
      .iter()
      .map(|row| meta(row))
      .collect::<Vec<_>>(),
    distinct.iter().map(meta).collect::<Vec<_>>()
  );

  let identical = [
    record("x", EditOperation::Write, None, None),
    record("x", EditOperation::Update, None, None),
    record("x", EditOperation::Delete, None, None),
  ];
  let survivors = last_edit_records(&identical);
  assert_eq!(survivors.len(), 1);
  assert_eq!(meta(survivors[0]), ("x", EditOperation::Delete, None, None));

  let aliases = [
    record("./a", EditOperation::Write, None, None),
    record("a", EditOperation::Update, None, None),
  ];
  assert_eq!(
    last_edit_records(&aliases)
      .iter()
      .map(|row| meta(row))
      .collect::<Vec<_>>(),
    vec![
      ("./a", EditOperation::Write, None, None),
      ("a", EditOperation::Update, None, None),
    ]
  );
}

#[test]
fn last_edit_records_probed_scales_with_one_probe_per_record() {
  for n in [100_usize, 1_000, 10_000] {
    let identical: Vec<EditRecord> = (0..n)
      .map(|index| {
        record(
          "same",
          if index + 1 == n {
            EditOperation::Delete
          } else {
            EditOperation::Write
          },
          None,
          None,
        )
      })
      .collect();
    let mut probes = 0_u64;
    let survivors = last_edit_records_probed(&identical, &mut probes);
    assert_eq!(probes, n as u64, "identical n={n}");
    assert_eq!(survivors.len(), 1);
    assert_eq!(
      meta(survivors[0]),
      ("same", EditOperation::Delete, None, None)
    );

    let distinct: Vec<EditRecord> = (0..n)
      .map(|index| record(&format!("p{index}"), EditOperation::Write, None, None))
      .collect();
    probes = 0;
    let survivors = last_edit_records_probed(&distinct, &mut probes);
    assert_eq!(probes, n as u64, "distinct n={n}");
    assert_eq!(survivors.len(), n);
    assert_eq!(survivors[0].path, "p0");
    assert_eq!(survivors[n - 1].path, format!("p{}", n - 1));
  }
}
