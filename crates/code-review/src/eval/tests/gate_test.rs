use super::violations;
use crate::eval::Gate;

#[test]
fn every_flag_holds_against_the_production_predicates() {
  let all = Gate {
    blocking_severities_drive_count: true,
    advisory_never_blocks: true,
    unreviewed_never_writes_pass: true,
  };
  assert_eq!(violations(&all), Vec::<String>::new());
}

#[test]
fn an_unflagged_gate_checks_nothing() {
  assert_eq!(violations(&Gate::default()), Vec::<String>::new());
}

#[test]
fn each_flag_alone_holds() {
  for gate in [
    Gate {
      blocking_severities_drive_count: true,
      ..Gate::default()
    },
    Gate {
      advisory_never_blocks: true,
      ..Gate::default()
    },
    Gate {
      unreviewed_never_writes_pass: true,
      ..Gate::default()
    },
  ] {
    assert_eq!(violations(&gate), Vec::<String>::new(), "{gate:?}");
  }
}
