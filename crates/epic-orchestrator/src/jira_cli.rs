//! `toolu epic jira issue get <KEY>` (`jira-issue.ts`): a worker reads its Jira
//! issue without the jira plugin.

use crate::tracker::jira_client::JiraClient;
use crate::tracker::parse_jira_ref;

/// The usage line every bad argument prints.
pub(crate) const USAGE: &str = "Usage: toolu epic jira issue get <KEY>";

/// The issue key `raw` names.
///
/// # Errors
/// It is no Jira key.
pub(crate) fn parse_key(raw: &str) -> Result<String, String> {
  parse_jira_ref(raw).ok_or_else(|| USAGE.to_owned())
}

/// The raw issue as pretty JSON with a trailing newline.
///
/// # Errors
/// The request fails.
pub(crate) fn issue_get(client: &JiraClient, key: &str) -> Result<String, String> {
  let issue = client.raw_issue(key)?;
  let mut text = serde_json::to_string_pretty(&issue).map_err(|err| err.to_string())?;
  text.push('\n');
  Ok(text)
}

#[cfg(test)]
#[path = "tests/jira_cli_test.rs"]
mod tests;
