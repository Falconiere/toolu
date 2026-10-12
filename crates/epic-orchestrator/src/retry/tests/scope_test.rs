use std::sync::Arc;

use super::{effect_scope, scoped_budget};
use crate::retry::{Budget, Policy};

const POLICY: Policy = Policy {
  attempts: 2,
  base_ms: 1,
  max_sleep_ms: 3000,
};

#[test]
fn clients_built_in_one_effect_share_its_budget() {
  let (first, second) = effect_scope(&POLICY, || (scoped_budget(), scoped_budget()));
  assert!(Arc::ptr_eq(&first, &second));
  assert_eq!(*first.lock().expect("lock"), Budget { remaining_ms: 3000 });
}

#[test]
fn outside_an_effect_each_client_gets_an_unbounded_budget() {
  let _inside = effect_scope(&POLICY, scoped_budget);
  let (first, second) = (scoped_budget(), scoped_budget());
  assert!(!Arc::ptr_eq(&first, &second));
  assert_eq!(*first.lock().expect("lock"), Budget::unbounded());
}
