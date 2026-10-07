use std::path::PathBuf;

use toolu_runtime::json::ordered::Ordered;
use toolu_runtime::registry::ModuleKind;

use super::{MARK, RUNNER, Request, request_text};
use crate::registry::Entry;

#[test]
fn the_request_carries_the_stop_kind_event_context_and_modules_in_order() {
  let event = Ordered::Object(vec![(
    "type".to_owned(),
    Ordered::String("tool/post".to_owned()),
  )]);
  let ctx = Ordered::Object(vec![(
    "host".to_owned(),
    Ordered::String("codex".to_owned()),
  )]);
  let request = Request {
    stop: "post_block",
    registry_event: "tool/post",
    event: &event,
    ctx: &ctx,
  };
  let module = Entry {
    file: "x@t__m.js".to_owned(),
    path: PathBuf::from("/r/x@t__m.js"),
    spec: "x@t".to_owned(),
    name: "m".to_owned(),
    kind: ModuleKind::Esm,
  };
  assert_eq!(
    request_text(&request, &[&module]),
    r#"{"stop":"post_block","registryEvent":"tool/post","event":{"type":"tool/post"},"ctx":{"host":"codex"},"modules":[{"file":"x@t__m.js","path":"/r/x@t__m.js","spec":"x@t","name":"m"}]}"#
  );
  assert!(RUNNER.contains(&format!("const MARK = \"{MARK}\";")));
}
