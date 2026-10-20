use std::path::Path;

use serde_json::json;

use super::{parse_inspect, parse_listing};

fn toolu_root() -> String {
  Path::new(env!("CARGO_MANIFEST_DIR"))
    .join("../../plugins/toolu")
    .display()
    .to_string()
}

#[test]
fn list_keeps_only_packages_from_the_toolu_repository() {
  let root = toolu_root();
  let body = json!({"plugins":[
    {"active":true,"plugin":{"id":"toolu"},
      "record":{"cache_path":root,"enabled":true,"trust":"user-local"}},
    {"active":true,"plugin":{"id":"other"},
      "record":{"cache_path":"/missing/other","enabled":true,"trust":"user-local"}}
  ]});
  let selected = parse_listing(&body.to_string()).unwrap();
  assert_eq!(selected.len(), 1);
  assert_eq!(selected[0].id, "toolu");
  assert!(parse_listing("{}").is_err());
  assert!(parse_listing("not json").is_err());
}

#[test]
fn inspect_distinguishes_unapproved_approved_and_unknown_hooks() {
  let unapproved = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"review_needed"}]});
  let pending = parse_inspect(&unapproved.to_string()).unwrap();
  assert_eq!(pending.pending_hooks, vec!["probe"]);
  assert_eq!(pending.approval, "needs-approval");
  let rejected = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"trusted_disabled"}]});
  let disabled = parse_inspect(&rejected.to_string()).unwrap();
  assert_eq!(disabled.pending_hooks, vec!["probe"]);
  assert_eq!(disabled.approval, "needs-approval");
  let approved = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"trusted_enabled"}]});
  assert_eq!(
    parse_inspect(&approved.to_string()).unwrap().approval,
    "approved"
  );
  let agent = json!({"runtime_capabilities":[{"candidate":{"scoped_definition_id":"toolu/implementer"},"status":"review_needed"}]});
  let pending_agent = parse_inspect(&agent.to_string()).unwrap();
  assert_eq!(pending_agent.approval, "needs-approval");
  assert!(pending_agent.pending_hooks.is_empty());
  let unknown = json!({"runtime_capabilities":[{"candidate":{"kind":"hook","capability_id":"probe"},"status":"future_status"}]});
  assert!(parse_inspect(&unknown.to_string()).is_err());
  assert!(parse_inspect("{}").is_err());
}
