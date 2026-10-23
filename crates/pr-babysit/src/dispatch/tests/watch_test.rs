use serde_json::json;

use super::*;
use crate::dispatch::sandbox::{Sandbox, sandbox};

fn waited(sb: &Sandbox, timeout: u64) -> Value {
  Dispatcher::new(&sb.options(false))
    .unwrap()
    .wait(timeout)
    .unwrap()
}

#[test]
fn without_a_round_wait_answers_none() {
  let sb = sandbox();
  assert_eq!(waited(&sb, 1), json!({"version": 1, "status": "none"}));
}

#[test]
fn a_group_names_the_host_its_fixer_is_stopped_as() {
  assert_eq!(host(&json!({"host": "muse"})), Some(Fixer::Muse));
  assert_eq!(host(&json!({"host": "claude"})), Some(Fixer::Claude));
  assert_eq!(host(&json!({"host": "vim"})), None);
  assert_eq!(host(&json!({"agent": "pb-1"})), None);
}
