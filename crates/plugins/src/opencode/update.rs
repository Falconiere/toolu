//! `update` on `OpenCode`: every toolu entry in scope rewritten to the target
//! spec, a tuple keeping its options. Names are checked against the catalog but
//! cannot narrow it: one package carries every plugin.

use crate::catalog::Catalog;
use crate::catalog::order::install_order;
use crate::fail::{Fail, Res};
use crate::opencode::entries::{PACKAGE, toolu_entries};
use crate::opencode::jsonc::{ConfigFile, replace_spec};
use crate::opencode::paths::Scope;
use crate::opencode::remove::{files_with_entries, package_scopes};
use crate::opencode::state::{Plan, State, Write};
use crate::step::{Status, Step};

fn update_file(file: &ConfigFile, target: &str) -> Res<Plan> {
  let mut text = file.text.clone().unwrap_or_default();
  let shown = file.path.display();
  let mut steps = Vec::new();
  for entry in toolu_entries(file)? {
    if entry.spec == target {
      steps.push(Step::new(PACKAGE, Status::Current, format!("current at {target} in {shown}"), Vec::new()));
      continue;
    }
    text = replace_spec(&text, entry.index, entry.tuple, target);
    let change = format!("{} -> {target} in {shown}", entry.spec);
    steps.push(
      Step::new(PACKAGE, Status::Updated, format!("updated {change}"), Vec::new()).planned(format!("update {change}")),
    );
  }
  let changed = steps.iter().any(|step| step.status == Status::Updated);
  let writes = if changed {
    vec![Write {
      path: file.path.clone(),
      text,
    }]
  } else {
    Vec::new()
  };
  Ok(Plan { steps, writes })
}

/// Rewrites every toolu entry of the scope to `target`.
pub(crate) fn plan(state: &State, catalog: &Catalog, names: &[String], scope: Option<Scope>, target: &str) -> Res<Plan> {
  if !names.is_empty() {
    install_order(catalog, names)?;
  }
  let files = files_with_entries(state, &package_scopes(state, scope)?)?;
  if files.is_empty() {
    let within = scope.map(|scope| format!(" ({} scope)", scope.name())).unwrap_or_default();
    return Err(Fail::failed(format!(
      "{PACKAGE} is not configured in OpenCode{within}, so there is nothing to update"
    )));
  }
  let mut plan = Plan::default();
  for file in files {
    let one = update_file(file, target)?;
    plan.steps.extend(one.steps);
    plan.writes.extend(one.writes);
  }
  Ok(plan)
}

#[cfg(test)]
#[path = "tests/update_test.rs"]
mod tests;
