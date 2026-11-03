use serde_json::json;

use super::fixture_test::{clean_case, env, invoke, options, review_with_finding, saved_state};
use super::pipeline_local_test::git_output;
use crate::contract::Verdict;

#[test]
fn reset_and_recommit_cannot_restore_a_stale_passing_review() {
  let (repo, state, clean) = clean_case();
  let root = repo.path();
  let clean_env = env(root, state.path(), &clean);
  assert_eq!(
    invoke(&clean_env, &options(&clean)).verdict(),
    Verdict::Approved
  );
  let prior = saved_state(state.path());
  assert_eq!(prior["findings_count"], 0);
  let prior_sha = prior["diff_sha"].as_str().unwrap().to_owned();
  git_output(root, &["reset", "--mixed", "main"]);

  let env = review_with_finding(root, state.path());
  let saved = saved_state(state.path());
  assert!(saved["findings_count"].as_u64().unwrap() > 0);
  let empty_sha = toolu_state::diff_sha::diff_sha(&env, root, "main").unwrap();
  assert_eq!(saved["diff_sha"], empty_sha);
  assert_eq!(saved["reviewed_files"], json!([]));

  git_output(root, &["add", "src.txt", "café.rs"]);
  git_output(root, &["commit", "-qm", "recommit"]);
  assert_eq!(
    toolu_state::diff_sha::diff_sha(&env, root, "main").unwrap(),
    prior_sha
  );
  assert_ne!(saved_state(state.path())["diff_sha"], prior_sha);
}
