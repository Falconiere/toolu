//! Scoped pre-check for the current branch.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::metadata::{self, Metadata};
use crate::options::Options;
use crate::{Verdict, gate, gate_change, output};

/// Run collected gate checks, then tests for changed crates and their dependents.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let root = options.root.as_path();
  let base_ref = options.base.as_deref().unwrap_or("origin/main");
  let base = gate_change::git(root, &["merge-base", base_ref, "HEAD"])?;
  let paths = gate_change::changed(root, base.trim())?;
  let selected = packages(&metadata::load(root)?, &paths);
  let mut gated = options.clone();
  gated.only = gate::STEPS
    .iter()
    .take(gate::COLLECTED)
    .map(|step| (*step).to_owned())
    .collect();
  if gate::run(&gated)? == Verdict::Findings {
    return Ok(Verdict::Findings);
  }
  let result = match selected {
    None => gate::cargo(
      root,
      &[
        "nextest",
        "run",
        "--locked",
        "--no-tests=pass",
        "--workspace",
      ],
    ),
    Some(names) if names.is_empty() => Ok(Verdict::Clean),
    Some(names) => {
      let mut args = vec!["nextest", "run", "--locked", "--no-tests=pass"];
      for name in &names {
        args.extend(["-p", name]);
      }
      gate::cargo(root, &args)
    }
  }?;
  if result == Verdict::Clean {
    output::say("check-changed: ok — run cargo xtask gate before pushing");
  }
  Ok(result)
}

/// `None` selects the workspace; otherwise select owning crates and linked dependents.
fn packages(metadata: &Metadata, paths: &[PathBuf]) -> Option<Vec<String>> {
  let members: Vec<_> = metadata.members().collect();
  let mut names = BTreeSet::new();
  for path in paths {
    let absolute = metadata.workspace_root.join(path);
    let owner = members
      .iter()
      .filter(|member| absolute.starts_with(member.dir()))
      .max_by_key(|member| member.dir().components().count())?;
    names.insert(owner.name.as_str());
  }
  loop {
    let before = names.len();
    let selected_dirs: BTreeSet<_> = members
      .iter()
      .filter(|member| names.contains(member.name.as_str()))
      .map(|member| member.dir())
      .collect();
    for member in &members {
      if member.dependencies.iter().any(|dep| {
        dep.is_linked()
          && dep
            .path
            .as_deref()
            .is_some_and(|path| selected_dirs.contains(path))
      }) {
        names.insert(member.name.as_str());
      }
    }
    if names.len() == before {
      break;
    }
  }
  Some(names.into_iter().map(str::to_owned).collect())
}

#[cfg(test)]
#[path = "tests/check_changed_test.rs"]
mod tests;
