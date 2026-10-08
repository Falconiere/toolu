//! `cargo xtask ci-aggregate`: judge `NEEDS` against a workflow's gated jobs (#458).

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::ci_model::load_repo;
use crate::options::Options;
use crate::{Verdict, output};

const CHANGES: &str = "changes";

struct JobNeed {
  result: String,
  outputs: Vec<(String, String)>,
}

/// What the aggregate concluded. A setup error stays `Err` (exit 2).
struct Report {
  ok: bool,
  lines: Vec<String>,
}

/// Judge one workflow from `NEEDS` and `CI_CHANGES_ROOT`.
pub(crate) fn run(options: &Options) -> Result<Verdict, String> {
  let Some(name) = one_workflow(options) else {
    return Ok(finding(
      "usage: cargo xtask ci-aggregate <workflow file name>",
    ));
  };
  let needs = std::env::var("NEEDS").ok();
  let report = evaluate(&aggregate_root(options), &name, needs.as_deref())?;
  Ok(emit(&report))
}

fn one_workflow(options: &Options) -> Option<String> {
  if options.files.len() != 1 {
    return None;
  }
  options
    .files
    .first()
    .map(|name| name.to_string_lossy().into_owned())
}

fn aggregate_root(options: &Options) -> PathBuf {
  std::env::var("CI_CHANGES_ROOT").map_or_else(|_| options.root.clone(), PathBuf::from)
}

/// Classify `workflow`. `needs` is the raw `NEEDS` text, absent when unset.
fn evaluate(root: &Path, workflow: &str, needs: Option<&str>) -> Result<Report, String> {
  let config = load_repo(root)?;
  let Some(spec) = config.workflows.iter().find(|(file, _)| file == workflow) else {
    return Ok(named(&format!("{workflow} has no entry in the data file")));
  };
  let parsed = match parsed_needs(needs) {
    Ok(parsed) => parsed,
    Err(message) => return Ok(named(&message)),
  };
  Ok(judge(&spec.1.jobs, &parsed))
}

fn parsed_needs(needs: Option<&str>) -> Result<Vec<(String, JobNeed)>, String> {
  let Some(raw) = needs.filter(|text| !text.is_empty()) else {
    return Err("NEEDS is not JSON".to_owned());
  };
  let value = serde_json::from_str::<Value>(raw).map_err(|_err| "NEEDS is not JSON".to_owned())?;
  parse_needs(&value).map_err(|()| "NEEDS is not the toJSON(needs) shape".to_owned())
}

fn parse_needs(value: &Value) -> Result<Vec<(String, JobNeed)>, ()> {
  let Some(obj) = value.as_object() else {
    return Err(());
  };
  let mut needs = Vec::new();
  for (id, job) in obj {
    needs.push((id.clone(), parse_job(job)?));
  }
  Ok(needs)
}

fn parse_job(value: &Value) -> Result<JobNeed, ()> {
  let Some(obj) = value.as_object() else {
    return Err(());
  };
  let Some(result) = obj.get("result").and_then(Value::as_str) else {
    return Err(());
  };
  Ok(JobNeed {
    result: result.to_owned(),
    outputs: parse_outputs(obj.get("outputs"))?,
  })
}

fn parse_outputs(value: Option<&Value>) -> Result<Vec<(String, String)>, ()> {
  let Some(value) = value else {
    return Ok(Vec::new());
  };
  let Some(obj) = value.as_object() else {
    return Err(());
  };
  let mut outputs = Vec::new();
  for (key, item) in obj {
    let Some(text) = item.as_str() else {
      return Err(());
    };
    outputs.push((key.clone(), text.to_owned()));
  }
  Ok(outputs)
}

fn judge(jobs: &[(String, String)], needs: &[(String, JobNeed)]) -> Report {
  let Some(changes) = need(needs, CHANGES) else {
    return plain(vec![format!("{CHANGES}: missing from needs")]);
  };
  if changes.result != "success" {
    return plain(vec![format!(
      "{CHANGES}: {}; no group decision to trust",
      changes.result
    )]);
  }
  let groups = unique_groups(jobs);
  let malformed = malformed_groups(&groups, &changes.outputs);
  if !malformed.is_empty() {
    return plain(malformed);
  }
  let mut failures = unmapped(jobs, needs);
  failures.extend(gated_failures(jobs, needs, &changes.outputs));
  if !failures.is_empty() {
    return plain(failures);
  }
  Report {
    ok: true,
    lines: summary(jobs, needs, &changes.outputs),
  }
}

fn unique_groups(jobs: &[(String, String)]) -> Vec<String> {
  let mut groups = Vec::new();
  for (_, group) in jobs {
    if !groups.iter().any(|seen: &String| seen == group) {
      groups.push(group.clone());
    }
  }
  groups
}

fn malformed_groups(groups: &[String], outputs: &[(String, String)]) -> Vec<String> {
  groups
    .iter()
    .filter(|group| !is_bool_output(outputs, group))
    .map(|group| {
      format!(
        "{CHANGES}: output {group} is \"{}\", not true or false",
        group_value(outputs, group)
      )
    })
    .collect()
}

fn is_bool_output(outputs: &[(String, String)], group: &str) -> bool {
  matches!(group_value(outputs, group), "true" | "false")
}

fn unmapped(jobs: &[(String, String)], needs: &[(String, JobNeed)]) -> Vec<String> {
  needs
    .iter()
    .filter(|(id, _)| id != CHANGES && !jobs.iter().any(|(job, _)| job == id))
    .map(|(id, _)| format!("{id}: needed but not mapped to a group"))
    .collect()
}

fn gated_failures(
  jobs: &[(String, String)],
  needs: &[(String, JobNeed)],
  outputs: &[(String, String)],
) -> Vec<String> {
  jobs
    .iter()
    .filter_map(|(id, group)| gated_line(id, group, need(needs, id), outputs))
    .collect()
}

fn gated_line(
  id: &str,
  group: &str,
  job: Option<&JobNeed>,
  outputs: &[(String, String)],
) -> Option<String> {
  let Some(job) = job else {
    return Some(format!("{id}: missing from needs"));
  };
  let on = group_value(outputs, group) == "true";
  let state = if on { "on" } else { "off" };
  let label = format!("{id} ({group} {state}): {}", job.result);
  if job.result == "success" {
    return None;
  }
  if job.result == "skipped" && on {
    return Some(format!("{label}, but its group is on"));
  }
  if job.result == "skipped" {
    return None;
  }
  Some(label)
}

fn summary(
  jobs: &[(String, String)],
  needs: &[(String, JobNeed)],
  outputs: &[(String, String)],
) -> Vec<String> {
  jobs
    .iter()
    .map(|(id, group)| {
      let state = if group_value(outputs, group) == "true" {
        "on"
      } else {
        "off"
      };
      let result = need(needs, id).map_or("missing", |job| job.result.as_str());
      format!("{id} ({group} {state}): {result}")
    })
    .collect()
}

fn need<'a>(needs: &'a [(String, JobNeed)], id: &str) -> Option<&'a JobNeed> {
  needs.iter().find(|(job, _)| job == id).map(|(_, job)| job)
}

fn group_value<'a>(outputs: &'a [(String, String)], group: &str) -> &'a str {
  match outputs.iter().find(|(key, _)| key == group) {
    Some((_, value)) => value.as_str(),
    None => "",
  }
}

fn named(message: &str) -> Report {
  Report {
    ok: false,
    lines: vec![format!("ci-aggregate: {message}")],
  }
}

fn plain(lines: Vec<String>) -> Report {
  Report { ok: false, lines }
}

fn finding(message: &str) -> Verdict {
  output::error(&format!("ci-aggregate: {message}"));
  Verdict::Findings
}

fn emit(report: &Report) -> Verdict {
  let text = report.lines.join("\n");
  if report.ok {
    output::say(&text);
    Verdict::Clean
  } else {
    output::error(&text);
    Verdict::Findings
  }
}

#[cfg(test)]
#[path = "tests/ci_aggregate_test.rs"]
mod tests;
