//! Full tracker descriptions for an initiating controller's remote brief.

use serde_json::{Value, json};
use toolu_runtime::env::Env;

use super::detect::{LinearRef, TrackerKind, parse_jira_ref, parse_linear_ref};
use super::gh::Gh;
use super::jira::adf_text;
use super::jira_client::JiraClient;
use super::linear::LinearTracker;
use crate::refs::parse_ref;

/// Fetch the full description of one child with local tracker credentials.
///
/// # Errors
/// The reference, credentials, tracker response, or request is invalid.
pub(crate) fn issue_description(
  kind: TrackerKind,
  reference: &str,
  env: &Env,
) -> Result<String, String> {
  match kind {
    TrackerKind::Github => github(&Gh::new(env)?, reference),
    TrackerKind::Jira => jira(&JiraClient::new(env)?, reference),
    TrackerKind::Linear => linear(&LinearTracker::new(reference, "", env)?, reference),
  }
}

fn text(value: Option<&Value>, source: &str) -> Result<String, String> {
  match value {
    Some(Value::Null) => Ok(String::new()),
    Some(Value::String(body)) => Ok(body.clone()),
    _ => Err(format!("{source}: invalid issue description")),
  }
}

fn github(gh: &Gh, reference: &str) -> Result<String, String> {
  let issue = parse_ref(reference, None)?;
  let body: Value = gh.get(&format!(
    "/repos/{}/{}/issues/{}",
    issue.owner, issue.repo, issue.number
  ))?;
  text(body.get("body"), "GitHub")
}

fn jira(client: &JiraClient, reference: &str) -> Result<String, String> {
  let key = parse_jira_ref(reference).ok_or("invalid Jira issue reference")?;
  let issue = client.issue(&key, &["description"])?;
  match issue.pointer("/fields/description") {
    Some(Value::Object(map)) if map.get("type").and_then(Value::as_str) == Some("doc") => {
      Ok(adf_text(
        issue
          .pointer("/fields/description")
          .ok_or("missing Jira description")?,
      ))
    }
    value => text(value, "Jira"),
  }
}

fn linear(client: &LinearTracker, reference: &str) -> Result<String, String> {
  let Some(LinearRef::Issue(id)) = parse_linear_ref(reference) else {
    return Err("invalid Linear issue reference".to_owned());
  };
  let data = client.gql(
    "query($id:String!){issue(id:$id){description}}",
    &json!({"id":id}),
    false,
  )?;
  text(data.pointer("/issue/description"), "Linear")
}

#[cfg(test)]
#[path = "tests/description_test.rs"]
mod tests;
