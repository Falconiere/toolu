//! Conditional REST probes for registered epic resources.

use std::collections::BTreeSet;

use serde_json::Value;
use toolu_github::{Client, Error, RateLimit, Rest};

use crate::watch::{Kind, Watch};

/// A GitHub change that the resident state thread can consume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Change {
  /// A PR, its checks, comments or reviews changed.
  PrChanged {
    key: String,
    head: String,
    base: String,
  },
  /// A PR was merged.
  PrMerged { key: String },
  /// A PR closed without a merge.
  PrClosed { key: String },
  /// A queued PR's base ref changed.
  BaseMoved { repo: String, branch: String },
  /// The epic or one of its open blockers changed.
  EpicChanged { key: String },
}

/// One watch's REST cost and inputs.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Report {
  /// Number of unconditional or changed representations.
  pub fresh: u64,
  /// Number of conditional `304` replies.
  pub not_modified: u64,
  /// REST primary points spent.
  pub rest_points: u64,
  /// Latest REST primary counters.
  pub rate: Option<RateLimit>,
  /// Changes for the state machine.
  pub changes: Vec<Change>,
}

/// Check one saved watch through `toolu-github`, updating its `ETag` values.
///
/// # Errors
/// The client, a malformed reply, or an incomplete pagination chain fails.
pub(crate) fn probe(client: &Client, watch: &mut Watch) -> Result<Report, Box<(Report, Error)>> {
  let mut report = Report::default();
  match probe_into(client, watch, &mut report) {
    Ok(()) => Ok(report),
    Err(err) => Err(Box::new((report, err))),
  }
}

fn probe_into(client: &Client, watch: &mut Watch, report: &mut Report) -> Result<(), Error> {
  let kind = watch.kind.clone();
  match &kind {
    Kind::Pr { .. } => pr(client, watch, report, &kind)?,
    Kind::Base { repo, branch } => base(client, watch, report, repo, branch)?,
    Kind::Epic { key, repo, number } => {
      let root = root(repo)?;
      let path = format!("{root}/issues/{number}/sub_issues?per_page=100");
      let _pages = pages(client, watch, report, &path, true)?;
      if report.fresh > 0 {
        report
          .changes
          .push(Change::EpicChanged { key: key.clone() });
      }
    }
    Kind::Blocker { key, repo, number } => {
      let root = root(repo)?;
      let path = format!("{root}/issues/{number}");
      let _reply = pages(client, watch, report, &path, false)?;
      if report.fresh > 0 {
        report
          .changes
          .push(Change::EpicChanged { key: key.clone() });
      }
    }
  }
  Ok(())
}

fn pr(client: &Client, watch: &mut Watch, report: &mut Report, kind: &Kind) -> Result<(), Error> {
  let Kind::Pr { key, repo, number } = kind else {
    return Err(Error::Config("expected a pull request watch".into()));
  };
  let root = root(repo)?;
  let path = format!("{root}/pulls/{number}");
  let before = report.fresh;
  if let Some(body) = pages(client, watch, report, &path, false)? {
    update_pr(watch, report, key, &body)?;
  }
  let head = watch.head_sha.clone();
  if head.is_empty() {
    return Err(Error::Decode("PR has no cached head SHA".into()));
  }
  for path in [
    format!("{root}/commits/{head}/check-runs?per_page=100"),
    format!("{root}/commits/{head}/status"),
  ] {
    let _reply = pages(client, watch, report, &path, false)?;
  }
  for path in [
    format!("{root}/issues/{number}/comments?per_page=100"),
    format!("{root}/pulls/{number}/comments?per_page=100"),
    format!("{root}/pulls/{number}/reviews?per_page=100"),
  ] {
    let _reply = pages(client, watch, report, &path, true)?;
  }
  if report.fresh > before {
    report.changes.push(Change::PrChanged {
      key: key.to_owned(),
      head: watch.head_sha.clone(),
      base: watch.base_ref.clone(),
    });
  }
  Ok(())
}

fn update_pr(watch: &mut Watch, report: &mut Report, key: &str, body: &Value) -> Result<(), Error> {
  let head = body.pointer("/head/sha").and_then(Value::as_str);
  let base = body.pointer("/base/ref").and_then(Value::as_str);
  let (Some(head), Some(base)) = (head, base) else {
    return Err(Error::Decode("PR reply has no head SHA or base ref".into()));
  };
  if !watch.head_sha.is_empty() && watch.head_sha != head {
    let old = format!("/commits/{}/", watch.head_sha);
    watch.etags.retain(|path, _| !path.contains(&old));
    watch.page_links.retain(|path, _| !path.contains(&old));
  }
  head.clone_into(&mut watch.head_sha);
  base.clone_into(&mut watch.base_ref);
  let merged = body.get("merged").and_then(Value::as_bool) == Some(true);
  let closed = body.get("state").and_then(Value::as_str) == Some("closed");
  if merged {
    report.changes.push(Change::PrMerged {
      key: key.to_owned(),
    });
  } else if closed {
    report.changes.push(Change::PrClosed {
      key: key.to_owned(),
    });
  }
  Ok(())
}

fn base(
  client: &Client,
  watch: &mut Watch,
  report: &mut Report,
  repo: &str,
  branch: &str,
) -> Result<(), Error> {
  let root = root(repo)?;
  if branch.is_empty() || !branch.bytes().all(safe_ref_byte) {
    return Err(Error::Config("invalid base ref".into()));
  }
  let path = format!("{root}/git/ref/heads/{branch}");
  let _reply = pages(client, watch, report, &path, false)?;
  if report.fresh > 0 {
    report.changes.push(Change::BaseMoved {
      repo: repo.to_owned(),
      branch: branch.to_owned(),
    });
  }
  Ok(())
}

fn safe_ref_byte(byte: u8) -> bool {
  byte.is_ascii_alphanumeric() || matches!(byte, b'/' | b'-' | b'_' | b'.')
}

fn root(repo: &str) -> Result<String, Error> {
  let (owner, name) = repo
    .split_once('/')
    .ok_or_else(|| Error::Config("repository must be owner/name".into()))?;
  let safe = |part: &str| {
    !part.is_empty()
      && part
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
  };
  if !safe(owner) || !safe(name) {
    return Err(Error::Config("repository must be owner/name".into()));
  }
  Ok(format!("/repos/{owner}/{name}"))
}

fn pages(
  client: &Client,
  watch: &mut Watch,
  report: &mut Report,
  path: &str,
  array: bool,
) -> Result<Option<Value>, Error> {
  let mut next = Some(path.to_owned());
  let mut first = None;
  let mut visited = BTreeSet::new();
  while let Some(current) = next.take() {
    if !visited.insert(current.clone()) || visited.len() > 1000 {
      return Err(Error::Decode("GitHub pagination did not terminate".into()));
    }
    let (body, following) = page(client, watch, report, &current, array)?;
    if first.is_none() {
      first = body;
    }
    next = following;
  }
  Ok(first)
}

fn page(
  client: &Client,
  watch: &mut Watch,
  report: &mut Report,
  current: &str,
  array: bool,
) -> Result<(Option<Value>, Option<String>), Error> {
  let etag = watch.etags.get(current).map(String::as_str);
  let reply = client.get(current, etag)?;
  report.rest_points = report
    .rest_points
    .saturating_add(reply.cost.points.unwrap_or(0));
  report.rate = Some(reply.cost.rate);
  match reply.data {
    Rest::NotModified => {
      report.not_modified = report.not_modified.saturating_add(1);
      Ok((None, watch.page_links.get(current).cloned()))
    }
    Rest::Fresh(fresh) => {
      let body: Value = fresh.json()?;
      if array && !body.is_array() {
        return Err(Error::Decode("GitHub page is not a JSON array".into()));
      }
      if !array && !body.is_object() {
        return Err(Error::Decode("GitHub page is not a JSON object".into()));
      }
      report.fresh = report.fresh.saturating_add(1);
      match fresh.etag {
        Some(etag) => {
          watch.etags.insert(current.to_owned(), etag);
        }
        None => {
          watch.etags.remove(current);
        }
      }
      let next = next_link(&fresh.headers);
      match &next {
        Some(link) => {
          watch.page_links.insert(current.to_owned(), link.clone());
        }
        None => {
          watch.page_links.remove(current);
        }
      }
      Ok((Some(body), next))
    }
  }
}

fn next_link(headers: &[(String, String)]) -> Option<String> {
  let links = headers.iter().find(|(name, _)| name == "link")?.1.as_str();
  for part in links.split(',') {
    let Some((url, relation)) = part.trim().split_once(';') else {
      continue;
    };
    if relation.trim() == "rel=\"next\"" {
      return url
        .trim()
        .strip_prefix('<')
        .and_then(|url| url.strip_suffix('>'))
        .map(str::to_owned);
    }
  }
  None
}

#[cfg(test)]
#[path = "tests/github_probe_test.rs"]
mod tests;
