//! `OpenCode`, which has no plugin commands on the supported host: the verbs
//! edit its documented config files instead (the former TypeScript installer's `src/opencode`).
//! One npm package, `@toolu/opencode`, carries every plugin; the verbs manage
//! its entry in a `plugin` array and the `toolu/plugins.json` selection.

use crate::args::{self, Request};
use crate::catalog::Catalog;
use crate::dispatch::Context;
use crate::fail::Res;
use crate::host::{Target, check_scope};
use crate::report::Row;
use crate::step::Step;

/// Plans become writes.
mod apply;
/// The package's entries and the host's merge.
pub(crate) mod entries;
/// `install`.
mod install;
/// Reading and editing JSONC.
pub(crate) mod jsonc;
/// `list`.
mod list;
/// Where the files are.
pub(crate) mod paths;
/// `remove`.
mod remove;
/// The plugin selection.
pub(crate) mod selection;
/// Line-keeping array edits.
mod splice;
/// What a run reads.
pub(crate) mod state;
/// `update`.
mod update;
/// Atomic writes.
mod write;

use entries::target_spec;
use paths::Scope;

/// `--scope user` is `OpenCode`'s global config, `project` the worktree root;
/// `local` is refused by the same check every host's scope goes through.
fn scope_of(scope: Option<args::Scope>) -> Res<Option<Scope>> {
  check_scope(scope, Target::Opencode)?;
  Ok(match scope {
    Some(args::Scope::User) => Some(Scope::Global),
    Some(args::Scope::Project) => Some(Scope::Project),
    Some(args::Scope::Local) | None => None,
  })
}

/// `install` on `OpenCode`.
pub(crate) fn install(
  request: &Request,
  names: &[String],
  catalog: &Catalog,
  cx: &Context<'_>,
) -> Res<Vec<Step>> {
  let state = state::load(&cx.cwd, &cx.env)?;
  let wanted = install::Install {
    names,
    scope: scope_of(request.scope)?,
    target: target_spec(&cx.env, cx.version)?,
  };
  apply::apply(
    &state,
    install::plan(&state, catalog, &wanted)?,
    request.dry_run,
  )
}

/// `list` on `OpenCode`: the header line and the rows.
pub(crate) fn list(catalog: &Catalog, cx: &Context<'_>) -> Res<(String, Vec<Row>)> {
  list::plan(&state::load(&cx.cwd, &cx.env)?, catalog)
}

/// `remove` on `OpenCode`.
pub(crate) fn remove(request: &Request, catalog: &Catalog, cx: &Context<'_>) -> Res<Vec<Step>> {
  let state = state::load(&cx.cwd, &cx.env)?;
  let plan = remove::plan(&state, catalog, &request.names, scope_of(request.scope)?)?;
  apply::apply(&state, plan, request.dry_run)
}

/// `update` on `OpenCode`.
pub(crate) fn update(request: &Request, catalog: &Catalog, cx: &Context<'_>) -> Res<Vec<Step>> {
  let state = state::load(&cx.cwd, &cx.env)?;
  let target = target_spec(&cx.env, cx.version)?;
  let plan = update::plan(
    &state,
    catalog,
    &request.names,
    scope_of(request.scope)?,
    &target,
  )?;
  apply::apply(&state, plan, request.dry_run)
}

#[cfg(test)]
#[path = "tests/opencode_test.rs"]
mod tests;
