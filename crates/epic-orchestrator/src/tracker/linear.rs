//! Linear epics over the GraphQL API (`scripts/trackers/linear.ts`): a parent
//! issue (children are its sub-issues) or a project (children are its issues).
//! A blocker is an inverse `blocks` relation.

use std::path::Path;

use serde_json::{Value, json};
use toolu_http::{Auth, Method};
use toolu_runtime::env::Env;

use super::Tracker;
use super::close_intent;
use super::detect::{LinearRef, TrackerKind, parse_linear_ref};
use super::linear_parse::{child, closed, linear_auth_header, lowest_completed};
use super::rest::Http;
use super::types::{EpicInfo, TrackedIssue, state_word};

const API: &str = "https://api.linear.app/graphql";
const CHILD_FIELDS: &str = "id identifier title url description state{type}\nlabels{nodes{name}}\ninverseRelations(first:50){nodes{type issue{identifier state{type}}}}";
const FINISH: &str = "query($id:String!){issue(id:$id){id state{type}\nteam{states(filter:{type:{eq:\"completed\"}}){nodes{id position}}}}}";
const COMMENT: &str =
  "mutation($i:String!,$b:String!){commentCreate(input:{issueId:$i,body:$b}){success}}";
const COMMENTS: &str = "query($id:String!,$after:String){issue(id:$id){comments(first:100,after:$after){nodes{body} pageInfo{hasNextPage endCursor}}}}";
const UPDATE: &str =
  "mutation($id:String!,$s:String!){issueUpdate(id:$id,input:{stateId:$s}){success}}";

/// A Linear epic and its transport.
pub(crate) struct LinearTracker {
  target: LinearRef,
  reference: String,
  default_repo: String,
  api: String,
  http: Http,
}

impl LinearTracker {
  /// The epic `reference` names; `LINEAR_API_KEY` holds a personal key or an
  /// OAuth token, and `LINEAR_API_URL` overrides the endpoint.
  ///
  /// # Errors
  /// The reference is no Linear issue or project, or the key is unset.
  pub(crate) fn new(reference: &str, default_repo: &str, env: &Env) -> Result<Self, String> {
    Self::with(reference, default_repo, env, Http::new(env, "Linear")?)
  }

  /// The epic over an existing transport.
  ///
  /// # Errors
  /// The reference is no Linear issue or project, or the key is unset.
  pub(super) fn with(
    reference: &str,
    default_repo: &str,
    env: &Env,
    http: Http,
  ) -> Result<Self, String> {
    let target = parse_linear_ref(reference)
      .ok_or_else(|| format!("not a Linear issue identifier or URL: {reference}"))?;
    let key = env
      .get("LINEAR_API_KEY")
      .ok_or("LINEAR_API_KEY is not set")?;
    let reference = match &target {
      LinearRef::Project(id) => format!("project:{id}"),
      LinearRef::Issue(id) => id.clone(),
    };
    Ok(Self {
      target,
      reference,
      default_repo: default_repo.to_owned(),
      api: env.get("LINEAR_API_URL").unwrap_or(API).to_owned(),
      http: http
        .redacting(key.trim())
        .authorized(Auth::Raw(linear_auth_header(key))),
    })
  }

  pub(super) fn gql(&self, query: &str, variables: &Value, write: bool) -> Result<Value, String> {
    let body = json!({"query": query, "variables": variables});
    let reply = self
      .http
      .json(Method::Post, &self.api, Some(&body), write)?;
    if let Some(errors) = reply
      .get("errors")
      .and_then(Value::as_array)
      .filter(|rows| !rows.is_empty())
    {
      let messages: Vec<&str> = errors
        .iter()
        .map(|error| error.get("message").and_then(Value::as_str).unwrap_or(""))
        .collect();
      return Err(format!("Linear: {}", messages.join("; ")));
    }
    reply
      .get("data")
      .filter(|data| !data.is_null())
      .cloned()
      .ok_or_else(|| "Linear: empty response".to_owned())
  }

  fn page(&self, field: &str, conn: &str, after: Option<&str>) -> Result<Value, String> {
    let query = format!(
      "query($id:String!,$after:String){{{field}(id:$id){{{conn}(first:100,after:$after){{\nnodes{{{CHILD_FIELDS}}} pageInfo{{hasNextPage endCursor}}}}}}}}"
    );
    let data = self.gql(&query, &json!({"id": self.id(), "after": after}), false)?;
    Ok(
      data
        .get(field)
        .and_then(|value| value.get(conn))
        .cloned()
        .unwrap_or(Value::Null),
    )
  }

  fn id(&self) -> &str {
    match &self.target {
      LinearRef::Issue(id) | LinearRef::Project(id) => id,
    }
  }

  fn has_comment(&self, id: &str, note: &str) -> Result<bool, String> {
    let mut after: Option<String> = None;
    for _ in 0..100 {
      let data = self.gql(COMMENTS, &json!({"id": id, "after": after}), false)?;
      let comments = data
        .pointer("/issue/comments")
        .ok_or("Linear: invalid comments response")?;
      let nodes = comments
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or("Linear: invalid comments response")?;
      let bodies = nodes
        .iter()
        .map(|node| {
          node
            .get("body")
            .and_then(Value::as_str)
            .ok_or("Linear: invalid comments response")
        })
        .collect::<Result<Vec<_>, _>>()?;
      if bodies.contains(&note) {
        return Ok(true);
      }
      let page = comments
        .get("pageInfo")
        .ok_or("Linear: invalid comments response")?;
      let more = page
        .get("hasNextPage")
        .and_then(Value::as_bool)
        .ok_or("Linear: invalid comments response")?;
      if !more {
        return Ok(false);
      }
      let next = page
        .get("endCursor")
        .and_then(Value::as_str)
        .filter(|next| !next.is_empty() && Some(*next) != after.as_deref())
        .ok_or("Linear: invalid comments cursor")?;
      after = Some(next.to_owned());
    }
    Err("Linear: comments exceed 100 pages".to_owned())
  }

  /// Comment on an open issue, then move it to its team's lowest completed
  /// state; a done issue gets no second comment.
  fn finish(
    &self,
    identifier: &str,
    note: &str,
    state_dir: &Path,
    role: &str,
  ) -> Result<String, String> {
    let data = self.gql(FINISH, &json!({"id": identifier}), false)?;
    let issue = data.get("issue").cloned().unwrap_or(Value::Null);
    let uuid = issue
      .get("id")
      .and_then(Value::as_str)
      .unwrap_or("")
      .to_owned();
    if uuid.is_empty() {
      return Err("Linear: invalid issue response".to_owned());
    }
    if closed(issue.get("state")) {
      return Ok("already-done".to_owned());
    }
    close_intent::comment(
      state_dir,
      ("linear", role, &uuid),
      note,
      |body| self.has_comment(&uuid, body),
      |body| {
        let reply = self.gql(COMMENT, &json!({"i": uuid, "b": body}), true)?;
        (reply.pointer("/commentCreate/success") == Some(&Value::Bool(true)))
          .then_some(())
          .ok_or_else(|| "Linear: commentCreate failed".to_owned())
      },
    )?;
    let target = lowest_completed(&issue);
    let Some(state) = target else {
      return Ok("no-completed-state".to_owned());
    };
    let reply = self.gql(UPDATE, &json!({"id": uuid, "s": state}), true)?;
    if reply.pointer("/issueUpdate/success") != Some(&Value::Bool(true)) {
      return Err("Linear: issueUpdate failed".to_owned());
    }
    Ok("completed".to_owned())
  }
}

impl Tracker for LinearTracker {
  fn kind(&self) -> TrackerKind {
    TrackerKind::Linear
  }

  fn epic(&self) -> Result<EpicInfo, String> {
    if let LinearRef::Project(id) = &self.target {
      let data = self.gql(
        "query($id:String!){project(id:$id){id name url completedAt}}",
        &json!({"id": id}),
        false,
      )?;
      let project = data.get("project").cloned().unwrap_or(Value::Null);
      let text = |key: &str| {
        project
          .get(key)
          .and_then(Value::as_str)
          .unwrap_or("")
          .to_owned()
      };
      return Ok(EpicInfo {
        reference: self.reference.clone(),
        title: text("name"),
        state: state_word(project.get("completedAt").is_some_and(|at| !at.is_null())),
        url: text("url"),
      });
    }
    let data = self.gql(
      "query($id:String!){issue(id:$id){id identifier title url state{type}}}",
      &json!({"id": self.id()}),
      false,
    )?;
    let issue = data.get("issue").cloned().unwrap_or(Value::Null);
    let text = |key: &str| {
      issue
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned()
    };
    Ok(EpicInfo {
      reference: self.reference.clone(),
      title: text("title"),
      state: state_word(closed(issue.get("state"))),
      url: text("url"),
    })
  }

  fn children(&self) -> Result<Vec<TrackedIssue>, String> {
    let (field, conn) = match self.target {
      LinearRef::Project(_) => ("project", "issues"),
      LinearRef::Issue(_) => ("issue", "children"),
    };
    let mut nodes = Vec::new();
    let mut after: Option<String> = None;
    for _ in 0..20 {
      let page = self.page(field, conn, after.as_deref())?;
      if page.is_null() {
        break;
      }
      nodes.extend(
        page
          .get("nodes")
          .and_then(Value::as_array)
          .cloned()
          .unwrap_or_default(),
      );
      let more = page
        .pointer("/pageInfo/hasNextPage")
        .and_then(Value::as_bool)
        == Some(true);
      after = page
        .pointer("/pageInfo/endCursor")
        .and_then(Value::as_str)
        .map(str::to_owned);
      if !more {
        break;
      }
    }
    Ok(
      nodes
        .iter()
        .map(|issue| child(issue, &self.default_repo))
        .collect(),
    )
  }

  fn is_closed(&self, reference: &str) -> Result<bool, String> {
    let data = self.gql(
      "query($id:String!){issue(id:$id){state{type}}}",
      &json!({"id": reference}),
      false,
    )?;
    Ok(closed(data.pointer("/issue/state")))
  }

  fn close_issue(&self, reference: &str, note: &str, state_dir: &Path) -> Result<String, String> {
    self.finish(reference, note, state_dir, "issue")
  }

  fn tick_epic(&self, _reference: &str, _title: &str) -> Result<bool, String> {
    Ok(false)
  }

  fn close_epic(&self, summary: &str, state_dir: &Path) -> Result<String, String> {
    match &self.target {
      // Project status is workspace-configured; leave the close to a human.
      LinearRef::Project(_) => Ok("project-left-open".to_owned()),
      LinearRef::Issue(id) => self.finish(id, summary, state_dir, "epic"),
    }
  }

  fn state_slug(&self) -> String {
    format!("linear-{}", self.reference.replace([':', '/'], "-")).to_lowercase()
  }
}

#[cfg(test)]
#[path = "tests/linear_test.rs"]
mod tests;
