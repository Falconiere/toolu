//! Feed fixed GitHub checks into the resident engine's state thread.

use std::time::Duration;

use serde_json::{Value, json};
use toolu_engine::babysit::BabysitTick;
use toolu_github::{Client, Config, Error};
use toolu_runtime::env::Env;

use crate::babysit;
use crate::babysit_fresh::{self, Babysit};
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
        note_error(&mut self.world, &key, cause, &err, &Report::default());
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
  let original = world.watches.get(key).cloned();
  let result = world
    .watches
    .get_mut(key)
    .map(|watched| github_probe::probe(checks.client, watched));
  match result {
    Some(Ok(report)) => return note_report(world, key, cause, report, checks),
    Some(Err(failure)) => {
      let (report, err) = *failure;
      if let Some(watch) = original {
        world.watches.insert(key.to_owned(), watch);
      }
      let limited = matches!(
        err,
        Error::RateLimited {
          retry_after: Some(_),
          ..
        }
      );
      note_error(world, key, cause, &err, &report);
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

fn note_report(
  world: &mut World,
  key: &str,
  cause: Cause,
  report: Report,
  checks: &Checks<'_>,
) -> bool {
  let rest_rate = report.rate.clone();
  let rest_points = report.rest_points;
  let quiet = cause == Cause::Scheduled && report.fresh == 0;
  for change in report.changes {
    note_change(world, change);
  }
  let babysit = babysit_fresh::run(world, key, quiet, checks.babysit, checks.paths);
  world.rest_points = world.rest_points.saturating_add(rest_points);
  if let Some(rate) = &rest_rate {
    world.rest_rate = Some(rate.clone());
  }
  let retry_after = observe_graphql(world, babysit.checked());
  let graphql = babysit.checked().and_then(|result| result.graphql);
  let note = json!({
    "cause": cause_name(cause),
    "restFresh": report.fresh,
    "restNotModified": report.not_modified,
    "restPoints": rest_points,
    "restRate": rest_rate,
    "graphqlTick": u8::from(babysit.checked().is_some()),
    "graphqlPoints": graphql.map(|usage| usage.points),
    "graphqlRemaining": world.graphql_remaining,
    "graphqlResetAt": world.graphql_reset_at,
    "graphqlRetryAfter": retry_after.map(|wait| wait.as_secs()),
  });
  let mut note = with_watch(world, key, note);
  babysit_outcome(&mut note, &babysit);
  journal(world, key, "github-check", &note.to_string());
  if let Babysit::Checked(checked) = babysit {
    crate::babysit_next::apply(world, key, checked.next);
  }
  if let Some(crate::watch::Kind::Pr { key: issue, .. }) =
    world.watches.get(key).map(|w| w.kind.clone())
  {
    crate::logic::arm_merge(world, &issue);
  }
  retry_after.is_some()
}

/// Add the babysit decision, or the skipped collection's time, to `note`.
fn babysit_outcome(note: &mut Value, babysit: &Babysit) {
  let Some(map) = note.as_object_mut() else {
    return;
  };
  match babysit {
    Babysit::Idle => {}
    Babysit::Checked(checked) => {
      map.insert("next".to_owned(), json!(checked.next.name()));
    }
    Babysit::Skipped { at_ms } => {
      map.insert("babysit".to_owned(), json!("skipped"));
      map.insert("babysitCollectedAt".to_owned(), json!(at_ms));
    }
  }
}

fn observe_graphql(world: &mut World, checked: Option<&babysit::Checked>) -> Option<Duration> {
  let graphql = checked.and_then(|result| result.graphql);
  if let Some(usage) = graphql {
    world.graphql_points = world.graphql_points.saturating_add(usage.points);
    world.graphql_remaining = usage.remaining;
    world.graphql_reset_at = usage.reset_at;
  }
  let throttle = checked.and_then(|result| result.throttle.as_ref());
  let retry_after = throttle.and_then(|limit| limit.retry_after);
  if let Some(limit) = throttle {
    world.graphql_remaining = limit.remaining;
    world.graphql_reset_at = limit.reset_at;
  }
  if let Some(wait) = retry_after {
    let millis = u64::try_from(wait.as_millis()).unwrap_or(u64::MAX);
    world.github_hold_until_ms = world
      .github_hold_until_ms
      .max(world.now_ms.saturating_add(millis));
  }
  retry_after
}

fn note_error(world: &mut World, key: &str, cause: Cause, err: &Error, report: &Report) {
  world.rest_points = world.rest_points.saturating_add(report.rest_points);
  if let Some(counters) = report.rate.as_ref() {
    world.rest_rate = Some(counters.clone());
  }
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
    world.rest_rate = Some(counters.clone());
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
    "restFresh": report.fresh, "restNotModified": report.not_modified,
    "restPoints": report.rest_points,
  });
  let note = with_watch(world, key, note);
  journal(world, key, "github-error", &note.to_string());
}

/// Apply one observed GitHub change to the epic world.
pub(crate) fn note_change(world: &mut World, change: Change) {
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
      journal(world, &key, "github-pr-merged", "");
      crate::merge_bridge::merged(world, &key);
    }
    Change::PrClosed { key } => {
      if let Some(issue) = world.issues.get_mut(&key) {
        "closed".clone_into(&mut issue.stage);
        world.outbox.push_back(Step::Issue { key: key.clone() });
      }
      push_attention(world, "pr-closed", &key, "pull request closed");
    }
    Change::BaseMoved { repo, branch } => {
      // A moved base can change mergeability with no PR ETag moving.
      for watch in world.watches.values_mut() {
        let same_repo =
          matches!(&watch.kind, crate::watch::Kind::Pr { repo: watched, .. } if *watched == repo);
        if same_repo && watch.base_ref == branch {
          watch.babysit = None;
        }
      }
      let note = format!("{repo}@{branch}");
      journal(world, &note, "github-base-moved", "");
      if let Some(head) = crate::queue::head(world, &repo) {
        crate::logic::arm_merge(world, &head);
      }
    }
    Change::EpicChanged { key } => {
      journal(world, &key, "github-epic-changed", "");
      crate::refresh::arm(world, &key);
    }
  }
}

/// `note` with the watch's issue (a pull request watch) and its next slot.
fn with_watch(world: &World, key: &str, mut note: Value) -> Value {
  let Some(watch) = world.watches.get(key) else {
    return note;
  };
  if let Some(map) = note.as_object_mut() {
    if let crate::watch::Kind::Pr { key: issue, .. } = &watch.kind {
      map.insert("issue".to_owned(), json!(issue));
    }
    map.insert("nextAt".to_owned(), json!(watch.next_at_ms));
  }
  note
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
#[path = "tests/github_engine_fixture_test.rs"]
mod fixture;

#[cfg(test)]
#[path = "tests/github_engine_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/github_engine_babysit_test.rs"]
mod babysit_tests;

#[cfg(test)]
#[path = "tests/github_engine_fresh_test.rs"]
mod fresh_tests;

#[cfg(test)]
#[path = "tests/github_engine_quiet_test.rs"]
mod quiet_tests;

#[cfg(test)]
#[path = "tests/github_engine_epic_test.rs"]
mod epic_tests;

#[cfg(test)]
#[path = "tests/github_engine_metrics_test.rs"]
mod metrics_tests;
