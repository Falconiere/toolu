//! Both ledger implementations over one [`Project`]: the TypeScript
//! `plan-ledger` bundle and `@toolu/core` modules through `bun`, and the
//! binary through `toolu ledger`.

use std::process::{Command, Output};

use serde_json::Value;

use crate::ledger::{Project, Res};

/// The repository root, for the bundle and the TypeScript modules.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// A spec with AC-1 and AC-2, and a plan of `steps` that names it.
pub(crate) fn plan(project: &Project, steps: &Value) -> Res<()> {
  project.write(
    "spec.md",
    "# S\n\n**Status:** Approved\n\n## Acceptance criteria\n- **AC-1:** a\n- **AC-2:** b\n",
  )?;
  project.write(
    "plan.md",
    &format!(
      "# P\n\n**Status:** Approved   **Spec:** spec.md\n\n## Steps (machine-readable)\n\n```json\n{steps}\n```\n"
    ),
  )
}

/// `bun <args>` in the repository with the project's environment.
pub(crate) fn bun(project: &Project, args: &[&str]) -> Command {
  let mut command = Command::new("bun");
  command
    .args(args)
    .current_dir(&project.root)
    .env_clear()
    .envs(project.env());
  command
}

/// The TypeScript `plan-ledger` bundle.
pub(crate) fn typescript(project: &Project, args: &[&str]) -> Res<Output> {
  let bundle = format!("{REPO}/plugins/toolu/hooks/dist/plan-ledger.js");
  Ok(bun(project, &[&bundle]).args(args).output()?)
}

/// `toolu ledger <args>`.
pub(crate) fn rust(project: &Project, args: &[&str]) -> Res<Output> {
  Ok(
    project
      .command(&project.root)
      .arg("ledger")
      .args(args)
      .output()?,
  )
}

/// `pointer` of the branch ledger, or null.
pub(crate) fn at(project: &Project, pointer: &str) -> Res<Value> {
  let ledger: Value = serde_json::from_str(&std::fs::read_to_string(project.ledger())?)?;
  Ok(ledger.pointer(pointer).cloned().unwrap_or(Value::Null))
}
