//! Feed fixed GitHub checks into the resident engine's state thread.

use serde_json::json;
use toolu_engine::babysit::{BabysitTick, TickRequest};
use toolu_github::{Client, Config, Error};
use toolu_runtime::env::Env;

use crate::babysit::{self, Next};
use crate::github_probe::{self, Change, Report};
use crate::journal::Record;
use crate::logic::push_attention;
use crate::model::{Step, World};
use crate::paths::Paths;
use crate::server::Engine;
use crate::watch::{self, Cause};

struct Checks<'a> {
  client: &'a Client,
  babysit: &'a dyn BabysitTick,
  paths: &'a Paths,
}

impl Engine {
  /// Run every due GitHub resource once without changing its fixed deadline.
  pub(crate) fn github_tick(&mut self, babysit: &dyn BabysitTick) {
    let due = watch::take_due(&mut self.world);
    if due.is_empty() {
      return;
    }
    if let Err(err) = self.ensure_github() {
      for (key, cause) in due {
        note_error(&mut self.world, &key, cause, &err);
      }
      return;
    }
    let Some(client) = self.github.as_ref() else {
      return;
    };
    let checks = Checks {
      client,
      babysit,
      paths: &self.paths,
    };
    for (position, (key, cause)) in due.iter().enumerate() {
      if check_one(&mut self.world, key, *cause, &checks) {
        rearm(&mut self.world, &due, position + 1);
        break;
      }
    }
    watch::sync(&mut self.world);
  }

  fn ensure_github(&mut self) -> Result<(), Error> {
    if self.github.is_none() {
      self.github = Some(Client::new(Config::scheduled(), &Env::process())?);
    }
    Ok(())
  }
}

fn check_one(world: &mut World, key: &str, cause: Cause, checks: &Checks<'_>) -> bool {
  let result = world
    .watches
    .get_mut(key)
    .map(|watched| github_probe::probe(checks.client, watched));
  match result {
    Some(Ok(report)) => note_report(world, key, cause, report, checks),
    Some(Err(err)) => {
      let limited = matches!(
        err,
        Error::RateLimited {
          retry_after: Some(_),
          ..
        }
      );
      note_error(world, key, cause, &err);
      return limited;
    }
    None => {}
  }
  false
}

fn rearm(world: &mut World, due: &[(String, Cause)], from: usize) {
  for (key, _) in due.iter().skip(from) {
    if let Some(watched) = world.watches.get_mut(key) {
      watched.request_immediate();
    }
  }
}

fn note_report(world: &mut World, key: &str, cause: Cause, report: Report, checks: &Checks<'_>) {
  let rest_rate = report.rate.clone();
  let rest_points = report.rest_points;
  for change in report.changes {
    note_change(world, change);
  }
  let checked = babysit_request(world, key, checks.paths)
    .map(|request| babysit::check(checks.babysit, &request));
  let graphql = checked.as_ref().and_then(|result| result.graphql);
  let note = json!({
    "cause": cause_name(cause),
    "restFresh": report.fresh,
    "restNotModified": report.not_modified,
    "restPoints": rest_points,
    "restRate": rest_rate,
    "graphqlTick": u8::from(checked.is_some()),
    "graphqlPoints": graphql.map(|usage| usage.points),
    "graphqlRemaining": graphql.and_then(|usage| usage.remaining),
  });
  journal(world, key, "github-check", &note.to_string());
  if let Some(checked) = checked {
    note_babysit(world, key, checked.next);
  }
}

fn babysit_request(world: &World, watch_key: &str, paths: &Paths) -> Option<TickRequest> {
  let watch = world.watches.get(watch_key)?;
  let crate::watch::Kind::Pr { key, repo, number } = &watch.kind else {
    return None;
  };
  let issue = world.issues.get(key)?;
  if !matches!(issue.phase.as_str(), "babysit" | "ready")
    || matches!(issue.stage.as_str(), "closed" | "merged")
  {
    return None;
  }
  Some(TickRequest {
    repo: repo.clone(),
    number: *number,
    state_file: paths
      .root
      .join("babysit")
      .join(format!("{}-{number}.json", repo.replace('/', "__"))),
    now: None,
  })
}

fn note_babysit(world: &mut World, watch_key: &str, next: Next) {
  let Some(crate::watch::Kind::Pr { key, .. }) =
    world.watches.get(watch_key).map(|watch| &watch.kind)
  else {
    return;
  };
  let key = key.clone();
  match next {
    Next::KeepGoing => {}
    Next::MergeQueue => {
      if let Some(issue) = world.issues.get_mut(&key)
        && issue.phase != "ready"
      {
        "ready".clone_into(&mut issue.phase);
        world.outbox.push_back(Step::Status { key: key.clone() });
        journal(world, &key, "github-babysit-ready", "");
      }
    }
    Next::Attention(note) => push_attention(world, "babysit", &key, &note),
  }
}

fn note_error(world: &mut World, key: &str, cause: Cause, err: &Error) {
  let mut retry = None;
  let mut secondary = false;
  let mut rate = None;
  if let Error::RateLimited {
    retry_after,
    rate: counters,
    secondary: is_secondary,
    ..
  } = err
  {
    retry = retry_after.map(|duration| duration.as_secs());
    secondary = *is_secondary;
    rate = Some(counters);
    if let Some(wait) = retry_after {
      let until = world
        .now_ms
        .saturating_add(wait.as_millis().try_into().unwrap_or(u64::MAX));
      world.github_hold_until_ms = world.github_hold_until_ms.max(until);
    }
  }
  let note = json!({
    "cause": cause_name(cause), "error": err.to_string(),
    "retryAfter": retry, "secondary": secondary, "rate": rate,
  });
  journal(world, key, "github-error", &note.to_string());
}

fn note_change(world: &mut World, change: Change) {
  match change {
    Change::PrChanged { key, head, base } => {
      if let Some(issue) = world.issues.get_mut(&key) {
        issue.base.clone_from(&base);
      }
      journal(
        world,
        &key,
        "github-pr-changed",
        &json!({"head":head,"base":base}).to_string(),
      );
    }
    Change::PrMerged { key } => {
      if let Some(issue) = world.issues.get_mut(&key) {
        "merged".clone_into(&mut issue.stage);
      }
      journal(world, &key, "github-pr-merged", "");
    }
    Change::PrClosed { key } => {
      if let Some(issue) = world.issues.get_mut(&key) {
        "closed".clone_into(&mut issue.stage);
      }
      push_attention(world, "pr-closed", &key, "pull request closed");
    }
    Change::BaseMoved { repo, branch } => {
      let note = format!("{repo}@{branch}");
      journal(world, &note, "github-base-moved", "");
    }
    Change::EpicChanged { key } => journal(world, &key, "github-epic-changed", ""),
  }
}

fn cause_name(cause: Cause) -> &'static str {
  match cause {
    Cause::Immediate => "immediate",
    Cause::Scheduled => "scheduled",
  }
}

fn journal(world: &mut World, key: &str, name: &str, note: &str) {
  world.outbox.push_back(Step::Journal(Record::new(
    "transition",
    name,
    key,
    "",
    note,
  )));
}

#[cfg(test)]
#[path = "tests/github_engine_test.rs"]
mod tests;
