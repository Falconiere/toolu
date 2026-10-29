use super::super::record::Record;
use super::transfer;
use crate::launch::Ctx;
use crate::paths::Paths;
use serde_json::json;
use toolu_runtime::env::Env;

#[test]
fn remote_context_requires_paired_saved_target() {
  let env = Env::from_pairs([("HOME", "/nonexistent")]);
  let paths = Paths::from_env(&env);
  let ctx = Ctx::live(&env, &paths, Err("offline".into()));
  let mut record = Record::new();
  assert!(transfer(&record, &ctx, "run-in").is_err());
  record.insert("machine".into(), json!("vps"));
  record.insert("remote_repo".into(), json!("/repo with spaces"));
  assert_eq!(
    transfer(&record, &ctx, "run-in").unwrap().target.repo,
    "/repo with spaces"
  );
}
