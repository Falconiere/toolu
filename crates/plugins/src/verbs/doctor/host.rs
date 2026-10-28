//! One host's inventory, setup, plugins, deps and secrets findings.

use std::collections::BTreeSet;

use toolu_runtime::host::roots::Roots;

use super::{Finding, HostReport, Scope, codex_pin, host_of, plugin, secrets};
use crate::args::Request;
use crate::catalog::Catalog;
use crate::cursor;
use crate::dispatch::{Context, driver};
use crate::host::Target;
use crate::host::adapter::Installed;

/// Why inventory could not be read; never includes host stderr.
pub(super) enum InventoryFail {
  /// Host CLI missing or cannot be prepared.
  Driver(String),
  /// Host list command failed.
  ListFailed,
  /// Host list output was malformed.
  Malformed(String),
}

/// Diagnoses `target`.
pub(super) fn diagnose(
  target: Target,
  catalog: &Catalog,
  request: &Request,
  cx: &Context<'_>,
) -> HostReport {
  let roots = Roots::new(cx.env.clone(), Some(host_of(target)));
  let (installed, inventory_err) = inventory(target, request, cx);
  let mut findings = setup_findings(target, inventory_err.as_ref(), &installed, cx);
  for entry in &installed {
    findings.extend(plugin::assess(target, entry, catalog, cx));
  }
  let names: BTreeSet<String> = installed.iter().map(|entry| entry.name.clone()).collect();
  findings.extend(plugin::deps(target, catalog, &names));
  findings.extend(secrets::check(&roots, &cx.env, &names));
  HostReport {
    host: target,
    findings,
  }
}

/// Inventory and marketplace findings before per-plugin checks.
pub(super) fn setup_findings(
  target: Target,
  inventory_err: Option<&InventoryFail>,
  installed: &[Installed],
  cx: &Context<'_>,
) -> Vec<Finding> {
  let mut findings = Vec::new();
  if let Some(err) = inventory_err {
    let (summary, fix) = match err {
      InventoryFail::Driver(detail) => (detail.clone(), None),
      InventoryFail::ListFailed => (
        "could not list its plugins (host command failed)".to_owned(),
        Some(format!("{} plugin list --json", target.name())),
      ),
      InventoryFail::Malformed(detail) => (format!("could not list its plugins: {detail}"), None),
    };
    findings.push(Finding::fail(Scope::Host, None, summary, fix));
  }
  if target == Target::Codex
    && let Some(notice) = codex_pin::notice(&cx.env, cx.version)
  {
    findings.push(Finding::warn(
      Scope::Host,
      Some("marketplace".to_owned()),
      notice,
      Some("toolu plugins update --host codex".to_owned()),
    ));
  }
  if installed.is_empty() && inventory_err.is_none() {
    findings.push(Finding::warn(
      Scope::Host,
      None,
      "no toolu plugin is installed",
      Some(format!("toolu plugins install --host {}", target.name())),
    ));
  }
  findings
}

fn inventory(
  target: Target,
  request: &Request,
  cx: &Context<'_>,
) -> (Vec<Installed>, Option<InventoryFail>) {
  if target == Target::Cursor {
    return (cursor_inventory(cx), None);
  }
  let driver = match driver(target, request, cx) {
    Ok(driver) => driver,
    Err(fail) => {
      return (Vec::new(), Some(InventoryFail::Driver(fail.message)));
    }
  };
  let argv = driver.adapter.list_installed();
  let ran = driver.run(&argv);
  if !ran.ok() {
    return (Vec::new(), Some(InventoryFail::ListFailed));
  }
  match driver.adapter.parse_list(&ran.stdout) {
    Ok(list) => (list, None),
    Err(cause) => (Vec::new(), Some(InventoryFail::Malformed(cause))),
  }
}

fn cursor_inventory(cx: &Context<'_>) -> Vec<Installed> {
  let store = cursor::store::Store::locate(&cx.env);
  store
    .ours()
    .into_iter()
    .map(|name| {
      let version = match store.state(&name) {
        cursor::store::State::Ours(version) => version.unwrap_or_else(|| "?".to_owned()),
        cursor::store::State::Absent | cursor::store::State::Foreign => "?".to_owned(),
      };
      Installed {
        name: name.clone(),
        version,
        enabled: true,
        source: Some(store.path(&name).display().to_string()),
      }
    })
    .collect()
}

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;
