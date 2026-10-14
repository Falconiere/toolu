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
