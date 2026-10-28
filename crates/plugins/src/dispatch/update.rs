//! `update`: one host with `--host`, as before; without it every detected host
//! that has toolu plugins, with no prompt. Each host's toolu plugins are read
//! before anything is prepared, pinned or cloned, so a host without them is
//! only noted, and that listing is the one the update then uses. A host that
//! fails, its listing included, is a failed step in its own section (exit 1)
//! and the others still run. `--scope` is checked with `--host` and otherwise
//! unused: Claude Code picks the scope of each update itself.

use crate::args::Request;
use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::dispatch::{Context, Report, driver, dry_text, failed};
use crate::fail::{Fail, Res};
use crate::host::adapter::Installed;
use crate::host::run::first_line;
use crate::host::{Target, check_scope, none_found, with_toolu};
use crate::local::store::Store;
use crate::local::{self, Local};
use crate::report::{self, rails};
use crate::step::{Status, Step};
use crate::verbs;

/// One host's toolu plugins as read before anything changes.
struct Have {
  /// Each plugin once.
  names: Vec<String>,
  /// The host's listing, for the update to reuse (none for Cursor and `OpenCode`).
  listing: Option<Vec<Installed>>,
}

/// Updates on `wanted`, or on every detected host when `--host` is absent.
pub(super) fn run(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  install_order(catalog, &request.names)?;
  let Some(target) = wanted else {
    return all(request, catalog, cx);
  };
  check_scope(request.scope, target)?;
  let have = Have {
    names: request.names.clone(),
    listing: None,
  };
  let steps = on(target, request, catalog, have, cx)?;
  Ok(Report {
    text: cx.shown(
      "update",
      |frame| rails::steps(frame, &[(target, steps.clone())], &[], request.dry_run),
      dry_text(request.dry_run, &steps).unwrap_or_else(|| report::update(&steps)),
    ),
    json: report::host_json(target, &steps, false),
    failed: failed(&steps),
  })
}

/// Updates `have.names` on `target`.
fn on(
  target: Target,
  request: &Request,
  catalog: &Catalog,
  have: Have,
  cx: &Context<'_>,
) -> Res<Vec<Step>> {
  match target {
    Target::Cursor | Target::Opencode => cx.spinning(target.name(), "copying plugins", |cx| {
      local::update(
        &Local::new(cx, target)?,
        catalog,
        &have.names,
        request.dry_run,
      )
    }),
    Target::Claude | Target::Codex | Target::Muse => verbs::update::run(
      &driver(target, request, cx)?,
      catalog,
      &have.names,
      request.dry_run,
      have.listing,
    ),
  }
}

/// The toolu plugins `target` reports, read without changing anything; why
/// they could not be read otherwise.
fn installed(target: Target, request: &Request, cx: &Context<'_>) -> Result<Have, String> {
  if matches!(target, Target::Cursor | Target::Opencode) {
    let names = Store::locate(&cx.env, target).ours();
    return Ok(Have {
      names,
      listing: None,
    });
  }
  let driver = driver(target, request, cx).map_err(|fail| fail.message)?;
  let ran = driver.run(&driver.adapter.list_installed());
  if !ran.ok() {
    return Err(first_line(&ran.stderr, "plugin list failed"));
  }
  let listing = driver.adapter.parse_list(&ran.stdout)?;
  Ok(Have {
    names: verbs::update::targets(&driver, &[], &listing),
    listing: Some(listing),
  })
}

/// What the prompt-free update makes of one host.
enum Done {
  /// Its steps, a failure included.
  Section(Vec<Step>),
  /// Why it was left out.
  Note(String),
}

/// Updates one host from what was read of it: the requested plugins it has,
/// a failed step when it could not be read, or a note when it has none.
fn host(
  target: Target,
  read: Result<Have, String>,
  request: &Request,
  catalog: &Catalog,
  cx: &Context<'_>,
) -> Done {
  let failed = |detail: String| {
    Done::Section(vec![Step::new(
      target.name(),
      Status::Failed,
      detail,
      Vec::new(),
    )])
  };
  let have = match read {
    Ok(have) => have,
    Err(cause) => return failed(format!("could not list its plugins: {cause}")),
  };
  let names: Vec<String> = if request.names.is_empty() {
    have.names.clone()
  } else {
    request
      .names
      .iter()
      .filter(|name| have.names.contains(name))
      .cloned()
      .collect()
  };
  if names.is_empty() {
    return Done::Note(skipped_note(&have.names, &request.names));
  }
  let work = Have {
    names,
    listing: have.listing,
  };
  match on(target, request, catalog, work, cx) {
    Ok(steps) => Done::Section(steps),
    Err(fail) => failed(fail.message),
  }
}

/// Every detected host: a section for each one with toolu plugins or a
/// failure, a note for each one without them.
fn all(request: &Request, catalog: &Catalog, cx: &mut Context<'_>) -> Res<Report> {
  let hosts = with_toolu(&cx.env);
  if hosts.is_empty() {
    return Err(none_found());
  }
  let found: Vec<(Target, Result<Have, String>)> = hosts
    .into_iter()
    .map(|target| (target, installed(target, request, cx)))
    .collect();
  anything_to_update(&found, &request.names)?;
  let mut sections = Vec::new();
  let mut notes = Vec::new();
  for (target, read) in found {
    match host(target, read, request, catalog, cx) {
      Done::Section(steps) => sections.push((target, steps)),
      Done::Note(note) => notes.push((target, note)),
    }
  }
  Ok(report(request, &sections, &notes, cx))
}

/// Refuses a run with nothing to update: a named plugin no host has, or no
/// toolu plugin anywhere (naming each host that could not be read).
fn anything_to_update(found: &[(Target, Result<Have, String>)], names: &[String]) -> Res<()> {
  let unread: Vec<String> = found
    .iter()
    .filter_map(|(target, read)| {
      read
        .as_ref()
        .err()
        .map(|cause| format!("{}: {cause}", target.name()))
    })
    .collect();
  let why = if unread.is_empty() {
    String::new()
  } else {
    format!(" (could not list: {})", unread.join("; "))
  };
  let has = |name: &String| {
    found
      .iter()
      .any(|(_, read)| read.as_ref().is_ok_and(|have| have.names.contains(name)))
  };
  if let Some(missing) = names.iter().find(|name| !has(name)) {
    return Err(Fail::failed(format!(
      "{missing} is not installed from toolu on any detected host{why}"
    )));
  }
  if found
    .iter()
    .all(|(_, read)| read.as_ref().map_or(true, |have| have.names.is_empty()))
  {
    return Err(Fail::failed(format!(
      "no detected host has toolu plugins installed, so there is nothing to update{why}"
    )));
  }
  Ok(())
}

/// Why a host was left out: it has no toolu plugins, or none of the names.
fn skipped_note(have: &[String], names: &[String]) -> String {
  if have.is_empty() {
    "no toolu plugins installed".to_owned()
  } else {
    format!("{} not installed", names.join(", "))
  }
}

/// A host section's plain body: the outcome marks, or on a dry run the commands
/// that would run followed by any failure, whose reason a dry run must show too.
fn plain_body(steps: &[Step], dry_run: bool) -> String {
  let (failures, rest): (Vec<Step>, Vec<Step>) = steps.iter().cloned().partition(Step::failed);
  if !dry_run || failures.is_empty() {
    return dry_text(dry_run, steps).unwrap_or_else(|| report::update(steps));
  }
  let planned = if rest.is_empty() {
    String::new()
  } else {
    dry_text(true, &rest).unwrap_or_default()
  };
  format!("{planned}{}", report::update(&failures))
}

fn report(
  request: &Request,
  sections: &[(Target, Vec<Step>)],
  notes: &[(Target, String)],
  cx: &Context<'_>,
) -> Report {
  let mut plain: Vec<String> = sections
    .iter()
    .map(|(target, steps)| format!("{}:\n{}", target.name(), plain_body(steps, request.dry_run)))
    .collect();
  plain.extend(
    notes
      .iter()
      .map(|(target, note)| format!("{}: {note}; skipped\n", target.name())),
  );
  let all: Vec<Step> = sections
    .iter()
    .flat_map(|(_, steps)| steps.clone())
    .collect();
  Report {
    text: cx.shown(
      "update",
      |frame| rails::steps(frame, sections, notes, request.dry_run),
      plain.join("\n"),
    ),
    json: report::install_json(sections, request.dry_run),
    failed: failed(&all),
  }
}

#[cfg(test)]
#[path = "tests/update_test.rs"]
mod tests;
