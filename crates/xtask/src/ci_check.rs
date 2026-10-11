//! `cargo xtask check-ci-paths`: workflows agree with `.github/ci-paths.json` (#458).

use std::path::Path;
use std::process::Command;

use crate::ci_model::{CHANGED, CiPaths, WorkflowSpec, load_repo};
use crate::ci_yaml::{Job, Workflow, read_workflows};
use crate::options::Options;
use crate::{Verdict, output};

const CHANGES: &str = "changes";

/// Run the static check.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let config = load_repo(&options.root)?;
  let tracked = tracked(&options.root)?;
  let read = read_workflows(&options.root.join(".github/workflows"))?;
  let mut problems = read.errors;
  problems.extend(check(&config, &read.workflows, &tracked));
  Ok(output::findings("check-ci-paths", &problems))
}

fn check(config: &CiPaths, workflows: &[Workflow], tracked: &[String]) -> Vec<String> {
  let mut problems = Vec::new();
  for (file, _) in &config.workflows {
    if !workflows.iter().any(|workflow| workflow.file == *file) {
      problems.push(format!("ci-paths.json: workflow {file} does not exist"));
    }
  }
  for workflow in workflows {
    let entry = config
      .workflows
      .iter()
      .find(|(file, _)| file == &workflow.file)
      .map(|(_, spec)| spec);
    if let Some(entry) = entry {
      problems.extend(check_workflow(entry, workflow, config));
    }
    problems.extend(check_unmapped(workflow, entry));
  }
  problems.extend(check_globs(config, tracked));
  problems
}

fn check_workflow(entry: &WorkflowSpec, workflow: &Workflow, config: &CiPaths) -> Vec<String> {
  let mut problems: Vec<String> = workflow
    .filtered
    .iter()
    .map(|trigger| {
      format!(
        "{}: reports a required check but filters {trigger} by paths",
        workflow.file
      )
    })
    .collect();
  for (id, group) in &entry.jobs {
    let job = workflow
      .jobs
      .iter()
      .find(|(name, _)| name == id)
      .map(|(_, job)| job);
    problems.extend(check_gated(&workflow.file, id, group, job, config));
  }
  for name in &entry.required {
    let found = workflow
      .jobs
      .iter()
      .any(|(id, job)| job.name.as_deref().unwrap_or(id) == name);
    if !found {
      problems.push(format!(
        "{}: required check {name} is not a job",
        workflow.file
      ));
    }
  }
  problems.extend(check_aggregate(&workflow.file, entry, workflow));
  problems.extend(check_changes(&workflow.file, entry, workflow));
  problems
}

fn check_gated(
  file: &str,
  id: &str,
  group: &str,
  job: Option<&Job>,
  config: &CiPaths,
) -> Vec<String> {
  let Some(job) = job else {
    return vec![format!("{file}: gated job {id} does not exist")];
  };
  let mut problems = Vec::new();
  if group != CHANGED && !config.group_defined(group) {
    problems.push(format!(
      "{file}: job {id} is mapped to undefined group {group}"
    ));
  }
  if !job.needs.iter().any(|need| need == CHANGES) {
    problems.push(format!("{file}: job {id} does not need {CHANGES}"));
  }
  let gate = format!("needs.changes.outputs.{group}");
  let positive = job.condition.contains(&format!("{gate} == 'true'"))
    || job.condition.contains(&format!("{gate} != 'false'"));
  if !positive {
    problems.push(format!("{file}: job {id} is not gated on {gate}"));
  }
  problems
}

fn check_aggregate(file: &str, entry: &WorkflowSpec, workflow: &Workflow) -> Vec<String> {
  let Some(name) = &entry.aggregate else {
    return Vec::new();
  };
  let Some(job) = workflow
    .jobs
    .iter()
    .find(|(id, _)| id == name)
    .map(|(_, job)| job)
  else {
    return vec![format!("{file}: aggregate {name} does not exist")];
  };
  let mut expected = vec![CHANGES.to_owned()];
  expected.extend(entry.jobs.iter().map(|(id, _)| id.clone()));
  let mut problems = Vec::new();
  if !same_set(&job.needs, &expected) {
    problems.push(format!(
      "{file}: aggregate {name} needs [{}], expected [{}]",
      job.needs.join(", "),
      expected.join(", ")
    ));
  }
  if !job.condition.contains("always()") {
    problems.push(format!(
      "{file}: aggregate {name} must run with if: always()"
    ));
  }
  problems
}

fn check_changes(file: &str, entry: &WorkflowSpec, workflow: &Workflow) -> Vec<String> {
  let Some(job) = workflow
    .jobs
    .iter()
    .find(|(id, _)| id == CHANGES)
    .map(|(_, job)| job)
  else {
    return vec![format!("{file}: has no {CHANGES} job")];
  };
  let mut seen = Vec::new();
  let mut missing = Vec::new();
  for (_, group) in &entry.jobs {
    if seen.iter().any(|item: &String| item == group) {
      continue;
    }
    seen.push(group.clone());
    if !job.outputs.iter().any(|output| output == group) {
      missing.push(format!("{file}: {CHANGES} job does not output {group}"));
    }
  }
  missing
}

fn check_unmapped(workflow: &Workflow, entry: Option<&WorkflowSpec>) -> Vec<String> {
  workflow
    .jobs
    .iter()
    .filter(|(id, job)| {
      !groups_read(&job.condition).is_empty()
        && entry.is_none_or(|spec| !spec.jobs.iter().any(|(job_id, _)| job_id == id))
    })
    .map(|(id, _)| {
      format!(
        "{}: job {id} reads needs.changes.outputs but has no group in the data file",
        workflow.file
      )
    })
    .collect()
}

fn groups_read(condition: &str) -> Vec<String> {
  let Ok(pattern) = regex::Regex::new(r"needs\.changes\.outputs\.([A-Za-z0-9_-]+)") else {
    return Vec::new();
  };
  pattern
    .captures_iter(condition)
    .filter_map(|caps| caps.get(1).map(|group| group.as_str().to_owned()))
    .collect()
}

fn check_globs(config: &CiPaths, tracked: &[String]) -> Vec<String> {
  config
    .glob_sources()
    .into_iter()
    .filter(|glob| !tracked.iter().any(|path| config.matches_glob(glob, path)))
    .map(|glob| format!("ci-paths.json: {glob} matches no tracked file"))
    .collect()
}

fn same_set(left: &[String], right: &[String]) -> bool {
  let mut left: Vec<&str> = left.iter().map(String::as_str).collect();
  let mut right: Vec<&str> = right.iter().map(String::as_str).collect();
  left.sort_unstable();
  right.sort_unstable();
  left.dedup();
  right.dedup();
  left == right
}

fn tracked(root: &Path) -> Result<Vec<String>, String> {
  let output = Command::new("git")
    .args(["-C"])
    .arg(root)
    .args(["ls-files", "-z"])
    .output()
    .map_err(|err| format!("git ls-files: {err}"))?;
  if !output.status.success() {
    return Err(format!("git ls-files exited {}", output.status));
  }
  Ok(
    String::from_utf8_lossy(&output.stdout)
      .split('\0')
      .filter(|path| !path.is_empty())
      .map(ToOwned::to_owned)
      .collect(),
  )
}

#[cfg(test)]
#[path = "tests/ci_check_test.rs"]
mod tests;
