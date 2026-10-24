//! Routes a request to its verb on each chosen host and turns the steps into a
//! report (the former TypeScript installer's `src/plugins/dispatch.ts`).

use std::time::Duration;

use serde_json::Value;
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use crate::args::{Request, Verb};
use crate::catalog::Catalog;
use crate::catalog::order::{install_order, removable};
use crate::cursor::{self, Cursor};
use crate::fail::{Fail, Res};
use crate::host::adapter::Adapter;
use crate::host::checkout::Checkout;
use crate::host::{Mode, Target, check_scope, resolve, target_of};
use crate::prompt::{Ask, Choice};
use crate::replay;
use crate::report;
use crate::step::{Status, Step};
use crate::verbs::{self, Driver};

/// What a run works with besides its request.
pub(crate) struct Context<'a> {
  /// The environment: `PATH` for host detection.
  pub(crate) env: Env,
  /// `--host`.
  pub(crate) host: Option<Host>,
  /// Prompts, when the run may ask.
  pub(crate) ask: Option<&'a mut dyn Ask>,
  /// The deadline of each host command.
  pub(crate) timeout: Duration,
  /// This binary's version: what `--config` is checked against, and the tag
  /// Muse's checkout follows and Codex's marketplace is pinned to.
  pub(crate) version: &'static str,
  /// The repository Muse's checkout is cloned from.
  pub(crate) origin: String,
}

/// A verb's result: the text, the `--json` document, and whether a step failed.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Report {
  /// The text report.
  pub(crate) text: String,
  /// The `--json` document.
  pub(crate) json: Value,
  /// Whether any step failed (exit 1).
  pub(crate) failed: bool,
}

fn driver(target: Target, request: &Request, cx: &Context<'_>) -> Res<Driver> {
  let (adapter, checkout) = match target {
    Target::Claude => (Adapter::Claude, None),
    Target::Codex => (Adapter::Codex, None),
    Target::Muse => (
      Adapter::Muse,
      Some(Checkout::locate(&cx.env, cx.version, &cx.origin)?),
    ),
    // Cursor has no plugin CLI; every verb branches to `crate::cursor` before here.
    Target::Cursor => return Err(Fail::failed("cursor has no plugin command to drive")),
  };
  let pin = (adapter == Adapter::Codex).then(|| format!("v{}", cx.version));
  Ok(Driver {
    adapter,
    env: cx.env.clone(),
    timeout: cx.timeout,
    checkout,
    pin,
    approval: request.approval,
  })
}

/// A dry run's report: the commands a real run would run (not those of a plugin
/// already current); `None` when the run was real.
fn dry_text(dry_run: bool, steps: &[Step]) -> Option<String> {
  if !dry_run {
    return None;
  }
  let would_run: Vec<Step> = steps
    .iter()
    .filter(|step| step.status == Status::Skipped)
    .cloned()
    .collect();
  Some(report::commands(&would_run))
}

/// The steps `--json` reports for a removal: a dry run removed nothing.
fn as_reported(steps: &[Step], dry_run: bool) -> Vec<Step> {
  steps
    .iter()
    .map(|step| match (dry_run, step.status) {
      (true, Status::Removed) => Step {
        status: Status::Skipped,
        ..step.clone()
      },
      _other => step.clone(),
    })
    .collect()
}

fn failed(steps: &[Step]) -> bool {
  steps.iter().any(Step::failed)
}

/// The hosts for `request`, with `--scope` checked against each.
fn hosts(
  request: &Request,
  wanted: Option<Target>,
  mode: Mode,
  cx: &mut Context<'_>,
) -> Res<Vec<Target>> {
  if let Some(target) = wanted {
    check_scope(request.scope, target)?;
  }
  let ask = cx.ask.as_deref_mut();
  let targets = resolve(wanted, mode, &cx.env, ask)?;
  for target in &targets {
    check_scope(request.scope, *target)?;
  }
  Ok(targets)
}

fn one_host(request: &Request, wanted: Option<Target>, cx: &mut Context<'_>) -> Res<Target> {
  let targets = hosts(request, wanted, Mode::One, cx)?;
  targets
    .first()
    .copied()
    .ok_or_else(|| Fail::usage("no host selected"))
}

/// Runs `request` against `catalog`.
pub(crate) fn dispatch(request: &Request, catalog: &Catalog, cx: &mut Context<'_>) -> Res<Report> {
  let mut wanted = cx.host.map(target_of).transpose()?;
  match request.verb {
    Verb::Install => {
      let replayed = match &request.config {
        Some(path) => Some(replay::read(path, cx.version)?),
        None => None,
      };
      if wanted.is_none() {
        wanted = replayed.as_ref().and_then(|selection| selection.host);
      }
      let names = replayed.map(|selection| selection.enabled);
      install(request, names, catalog, wanted, cx)
    }
    Verb::Remove => remove(request, catalog, wanted, cx),
    Verb::List => list(request, catalog, wanted, cx),
    Verb::Update => update(request, catalog, wanted, cx),
  }
}

/// The plugin picker: every catalog plugin, all selected.
fn pick(catalog: &Catalog, ask: &mut (dyn Ask + '_)) -> Res<Vec<String>> {
  let options: Vec<Choice> = catalog
    .plugins
    .iter()
    .map(|entry| (entry.name.clone(), entry.description.clone()))
    .collect();
  let chosen = ask.many("Which plugins should be installed?", &options)?;
  Ok(
    chosen
      .into_iter()
      .filter_map(|at| options.get(at).map(|(name, _)| name.clone()))
      .collect(),
  )
}

fn install(
  request: &Request,
  replayed: Option<Vec<String>>,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  let targets = hosts(request, wanted, Mode::Many, cx)?;
  let names = match (replayed, cx.ask.as_deref_mut()) {
    (Some(names), _any) => Some(names),
    (None, Some(ask)) if request.names.is_empty() => Some(pick(catalog, ask)?),
    (None, _no_prompt) => None,
  };
  let names = names.unwrap_or_else(|| request.names.clone());
  let nothing = request.config.is_some() && names.is_empty();
  let mut sections = Vec::new();
  for target in targets {
    let steps = if nothing {
      Vec::new()
    } else {
      match target {
        Target::Cursor => cursor::install(&Cursor::new(cx)?, catalog, &names, request.dry_run)?,
        Target::Claude | Target::Codex | Target::Muse => {
          let driver = driver(target, request, cx)?;
          verbs::install::run(&driver, catalog, &names, request.scope, request.dry_run)?
        }
      }
    };
    sections.push((target, steps));
  }
  let all: Vec<Step> = sections
    .iter()
    .flat_map(|(_, steps)| steps.clone())
    .collect();
  Ok(Report {
    text: report::install(&sections, request.dry_run),
    json: report::install_json(&sections, request.dry_run),
    failed: failed(&all),
  })
}

fn remove(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  if let Some(target) = wanted {
    check_scope(request.scope, target)?;
  }
  if request.names.is_empty() {
    return Err(Fail::usage("remove requires at least one name"));
  }
  if !request.yes {
    return Err(Fail::usage("remove requires --yes to confirm"));
  }
  let target = one_host(request, wanted, cx)?;
  removable(catalog, &request.names, target.removes_only_toolu())?;
  let steps = match target {
    Target::Cursor => cursor::remove(&Cursor::new(cx)?, &request.names, request.dry_run),
    Target::Claude | Target::Codex | Target::Muse => verbs::remove::run(
      &driver(target, request, cx)?,
      &request.names,
      request.dry_run,
    ),
  };
  Ok(Report {
    text: dry_text(request.dry_run, &steps).unwrap_or_else(|| report::remove(&steps)),
    json: report::host_json(target, &as_reported(&steps, request.dry_run), true),
    failed: failed(&steps),
  })
}

fn list(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  let target = one_host(request, wanted, cx)?;
  let rows = match target {
    Target::Cursor => cursor::list(&Cursor::new(cx)?, catalog),
    Target::Claude | Target::Codex | Target::Muse => {
      verbs::list::run(&driver(target, request, cx)?, catalog)?
    }
  };
  Ok(Report {
    text: report::list(&rows),
    json: report::rows_json(&rows),
    failed: false,
  })
}

fn update(
  request: &Request,
  catalog: &Catalog,
  wanted: Option<Target>,
  cx: &mut Context<'_>,
) -> Res<Report> {
  let target = one_host(request, wanted, cx)?;
  install_order(catalog, &request.names)?;
  let steps = match target {
    Target::Cursor => cursor::update(&Cursor::new(cx)?, &request.names, request.dry_run)?,
    Target::Claude | Target::Codex | Target::Muse => verbs::update::run(
      &driver(target, request, cx)?,
      &request.names,
      request.dry_run,
    )?,
  };
  Ok(Report {
    text: dry_text(request.dry_run, &steps).unwrap_or_else(|| report::update(&steps)),
    json: report::host_json(target, &steps, false),
    failed: failed(&steps),
  })
}

#[cfg(test)]
#[path = "tests/dispatch_test.rs"]
mod tests;
