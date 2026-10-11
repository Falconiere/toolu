use super::{pr_page, replies, round, thread_pages};

#[test]
fn the_rebuilt_pr_reply_carries_the_recorded_checks_as_graphql_contexts() {
  let page = pr_page();
  let pr = &page["data"]["repository"]["pullRequest"];
  assert_eq!(pr["author"]["login"], "Falconiere");
  let contexts = &pr["commits"]["nodes"][0]["commit"]["statusCheckRollup"]["contexts"]["nodes"];
  assert!(contexts[0]["checkSuite"]["workflowRun"]["workflow"]["name"].is_string());
  assert!(contexts[0].get("workflowName").is_none());
  assert_eq!(thread_pages().len(), 9);
  assert_eq!(round().len(), 10);
  assert_eq!(replies(&round()).len(), 10);
}
