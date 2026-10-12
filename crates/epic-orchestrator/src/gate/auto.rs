//! GitHub auto-merge pinned to the verified head: armed only while a PR waits
//! on checks alone, disarmed before any worker push (`rebase`, `fix`).

use serde_json::{Map, Value, json};

use super::assess::Facts;
use crate::tracker::Gh;

const ARM: &str = "mutation($id:ID!,$m:PullRequestMergeMethod!,$h:GitObjectID!){enablePullRequestAutoMerge(input:{pullRequestId:$id,mergeMethod:$m,expectedHeadOid:$h}){clientMutationId}}";
const DISARM: &str =
  "mutation($id:ID!){disablePullRequestAutoMerge(input:{pullRequestId:$id}){clientMutationId}}";

/// What to do with auto-merge after a verdict.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
  /// Arm it at the verified head.
  Arm,
  /// Disarm it before the worker pushes.
  Disarm,
}

/// Arm only a PR waiting on checks; disarm whenever the worker is about to push.
pub(crate) fn action(verdict: &str, armed: bool, wanted: bool) -> Option<Action> {
  if armed && matches!(verdict, "rebase" | "fix") {
    return Some(Action::Disarm);
  }
  (wanted && !armed && verdict == "wait").then_some(Action::Arm)
}

/// `armed`, `off`, or `unavailable` (arming was tried and refused) after a run.
pub(crate) fn state(result: &Map<String, Value>) -> &'static str {
  let flag = |key: &str| result.get(key).and_then(Value::as_bool);
  if flag("auto_merge_disarmed") == Some(true) {
    return "off";
  }
  match flag("auto_merge") {
    Some(true) => "armed",
    Some(false) => "unavailable",
    None if flag("auto_merge_armed") == Some(true) => "armed",
    None => "off",
  }
}

/// Arm auto-merge at `head`; a refusal is recorded, not raised.
pub(crate) fn arm(gh: &Gh, facts: &Facts, head: &str, method: &str) -> Map<String, Value> {
  let vars = json!({"id": facts.id, "m": method.to_uppercase(), "h": head});
  let mut out = Map::new();
  match gh.graphql(ARM, &vars, true) {
    Ok(_) => {
      out.insert("auto_merge".to_owned(), json!(true));
      out.insert("method".to_owned(), json!(method));
    }
    Err(err) => {
      out.insert("auto_merge".to_owned(), json!(false));
      out.insert("auto_merge_error".to_owned(), json!(err));
    }
  }
  out
}

/// Disarm auto-merge.
///
/// # Errors
/// GitHub refuses.
pub(crate) fn disarm(gh: &Gh, facts: &Facts) -> Result<(), String> {
  gh.graphql(DISARM, &json!({"id": facts.id}), true)
    .map(|_| ())
}

#[cfg(test)]
#[path = "tests/auto_test.rs"]
mod tests;
