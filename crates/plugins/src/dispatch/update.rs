//! `update`: one host with `--host`, as before; without it every detected host
//! that has toolu plugins, with no prompt. Each host's toolu plugins are read
//! before anything is prepared, pinned or cloned, so a host without them is
//! only noted. A host that fails is reported in its own section and the others
//! still run. `--scope` concerns Claude Code alone and is ignored elsewhere.

use crate::args::Request;
use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::cursor::store::Store;
use crate::cursor::{self, Cursor};
use crate::dispatch::{Context, Report, driver, dry_text, failed, one_host};
use crate::fail::{Fail, Res};
use crate::host::{Target, with_toolu};
use crate::report::{self, rails};
use crate::step::{Status, Step};
use crate::verbs;

/// Updates on `wanted`, or on every detected host when `--host` is absent.
pub(super) fn run(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  install_order(catalog, &request.names)?;
  let Some(_named) = wanted else {
    return all(request, catalog, cx);
  };
  let target = one_host(request, wanted, cx)?;
  let steps = on(target, request, catalog, &request.names, cx)?;
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

/// Updates `names` on `target`.
fn on(
  target: Target,
  request: &Request,
  catalog: &Catalog,
  names: &[String],
  cx: &Context<'_>,
) -> Res<Vec<Step>> {
  match target {
    Target::Cursor => cx.spinning("cursor", "copying plugins", |cx| {
      cursor::update(&Cursor::new(cx)?, catalog, names, request.dry_run)
    }),
    Target::Claude | Target::Codex | Target::Muse => verbs::update::run(
      &driver(target, request, cx)?,
      catalog,
      names,
      request.dry_run,
    ),
  }
}

/// The toolu plugins `target` reports, read without changing anything.
fn installed(target: Target, request: &Request, cx: &Context<'_>) -> Vec<String> {
  if target == Target::Cursor {
    return Store::locate(&cx.env).ours();
  }
  let Ok(driver) = driver(target, request, cx) else {
    return Vec::new();
  };
  let listing = driver.versions(&driver.adapter.list_installed());
  let mut names: Vec<String> = Vec::new();
  for entry in listing.iter().filter(|entry| !driver.foreign(entry)) {
    if !names.contains(&entry.name) {
      names.push(entry.name.clone());
    }
  }
  names
}

/// Every detected host with toolu plugins; the others are only noted.
fn all(request: &Request, catalog: &Catalog, cx: &mut Context<'_>) -> Res<Report> {
  let hosts = with_toolu(&cx.env);
  if hosts.is_empty() {
    return Err(Fail::usage(
      "no host found on PATH (probed claude, codex, muse). Choose one with --host.",
    ));
  }
  let found: Vec<(Target, Vec<String>)> = hosts
    .into_iter()
    .map(|target| (target, installed(target, request, cx)))
    .collect();
  anything_to_update(&found, &request.names)?;
  let mut sections = Vec::new();
  let mut notes = Vec::new();
  for (target, have) in found {
    let names: Vec<String> = if request.names.is_empty() {
      have.clone()
    } else {
      request
        .names
        .iter()
        .filter(|name| have.contains(name))
        .cloned()
        .collect()
    };
    if names.is_empty() {
      notes.push((target, skipped_note(&have, &request.names)));
      continue;
    }
    let steps = on(target, request, catalog, &names, cx).unwrap_or_else(|fail| {
      vec![Step::new(
        target.name(),
        Status::Failed,
        fail.message,
        Vec::new(),
      )]
    });
    sections.push((target, steps));
  }
  Ok(report(request, &sections, &notes, cx))
}

/// Refuses a run with nothing to update: a named plugin no host has, or no
/// toolu plugin anywhere.
fn anything_to_update(found: &[(Target, Vec<String>)], names: &[String]) -> Res<()> {
  if let Some(missing) = names
    .iter()
    .find(|name| !found.iter().any(|(_, have)| have.contains(name)))
  {
    return Err(Fail::failed(format!(
      "{missing} is not installed from toolu on any detected host"
    )));
  }
  if found.iter().all(|(_, have)| have.is_empty()) {
    return Err(Fail::failed(
      "no detected host has toolu plugins installed, so there is nothing to update",
    ));
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

fn report(
  request: &Request,
  sections: &[(Target, Vec<Step>)],
  notes: &[(Target, String)],
  cx: &Context<'_>,
) -> Report {
  let mut plain: Vec<String> = sections
    .iter()
    .map(|(target, steps)| {
      let body = dry_text(request.dry_run, steps).unwrap_or_else(|| report::update(steps));
      format!("{}:\n{body}", target.name())
    })
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
