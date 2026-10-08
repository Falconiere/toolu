//! Workflow YAML, parsed through Bun so the gate adds no YAML crate.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;

/// A parsed workflow job.
#[derive(Debug)]
pub(crate) struct Job {
  pub(crate) name: Option<String>,
  pub(crate) needs: Vec<String>,
  pub(crate) condition: String,
  pub(crate) outputs: Vec<String>,
}

/// One workflow file.
#[derive(Debug)]
pub(crate) struct Workflow {
  pub(crate) file: String,
  pub(crate) jobs: Vec<(String, Job)>,
  pub(crate) filtered: Vec<String>,
}

/// Workflows that parsed, plus one error line per file that did not.
pub(crate) struct Workflows {
  pub(crate) workflows: Vec<Workflow>,
  pub(crate) errors: Vec<String>,
}

/// Read every workflow in `dir`. A missing directory is an error; a bad file is a finding.
pub(crate) fn read_workflows(dir: &Path) -> Result<Workflows, String> {
  let mut names = std::fs::read_dir(dir)
    .map_err(|err| format!("cannot list {}: {err}", dir.display()))?
    .filter_map(Result::ok)
    .map(|entry| entry.file_name().to_string_lossy().into_owned())
    .filter(|name| yaml_file(name))
    .collect::<Vec<_>>();
  names.sort();
  let mut workflows = Vec::new();
  let mut errors = Vec::new();
  for name in names {
    match read_one(dir, &name) {
      Ok(workflow) => workflows.push(workflow),
      Err(err) => errors.push(err),
    }
  }
  Ok(Workflows { workflows, errors })
}

fn read_one(dir: &Path, name: &str) -> Result<Workflow, String> {
  let text = std::fs::read_to_string(dir.join(name))
    .map_err(|err| format!("{name}: not valid YAML: {err}"))?;
  let value = yaml_to_json(&text).map_err(|err| format!("{name}: not valid YAML: {err}"))?;
  parse_workflow(name, &value).map_err(|err| format!("{name}: not a workflow: {err}"))
}

fn parse_workflow(file: &str, value: &Value) -> Result<Workflow, String> {
  let obj = value.as_object().ok_or("root is not an object")?;
  let jobs_value = obj
    .get("jobs")
    .and_then(Value::as_object)
    .ok_or("jobs is not an object")?;
  let mut jobs = Vec::new();
  for (id, job) in jobs_value {
    jobs.push((id.clone(), parse_job(job)?));
  }
  Ok(Workflow {
    file: file.to_owned(),
    jobs,
    filtered: path_filters(obj.get("on").or_else(|| obj.get("true"))),
  })
}

fn parse_job(value: &Value) -> Result<Job, String> {
  let obj = value.as_object().ok_or("job is not an object")?;
  let name = obj
    .get("name")
    .and_then(Value::as_str)
    .map(ToOwned::to_owned);
  let needs = match obj.get("needs") {
    None => Vec::new(),
    Some(Value::String(one)) => vec![one.clone()],
    Some(Value::Array(items)) => {
      let mut needs = Vec::new();
      for item in items {
        let Some(text) = item.as_str() else {
          return Err("needs is not a string or list".to_owned());
        };
        needs.push(text.to_owned());
      }
      needs
    }
    Some(_) => return Err("needs is not a string or list".to_owned()),
  };
  let condition = match obj.get("if") {
    None => String::new(),
    Some(Value::String(text)) => text.clone(),
    Some(other) => other.to_string(),
  };
  let outputs = match obj.get("outputs") {
    None => Vec::new(),
    Some(Value::Object(outputs)) => outputs.keys().cloned().collect(),
    Some(_) => return Err("outputs is not an object".to_owned()),
  };
  Ok(Job {
    name,
    needs,
    condition,
    outputs,
  })
}

fn path_filters(on: Option<&Value>) -> Vec<String> {
  let Some(Value::Object(triggers)) = on else {
    return Vec::new();
  };
  triggers
    .iter()
    .filter_map(|(trigger, config)| {
      let object = config.as_object()?;
      (object.contains_key("paths") || object.contains_key("paths-ignore"))
        .then_some(trigger.clone())
    })
    .collect()
}

fn yaml_to_json(text: &str) -> Result<Value, String> {
  let mut child = Command::new(bun_binary()?)
    .args(["-e", "const fs=require('fs'); const text=fs.readFileSync(0,'utf8'); process.stdout.write(JSON.stringify(Bun.YAML.parse(text)))"])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|err| format!("cannot run bun: {err}"))?;
  {
    let mut stdin = child.stdin.take().ok_or("bun stdin is closed")?;
    stdin
      .write_all(text.as_bytes())
      .map_err(|err| format!("cannot write workflow to bun: {err}"))?;
  }
  let output = child
    .wait_with_output()
    .map_err(|err| format!("bun failed: {err}"))?;
  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).trim().to_owned());
  }
  serde_json::from_slice(&output.stdout).map_err(|err| format!("bun did not print JSON: {err}"))
}

fn yaml_file(name: &str) -> bool {
  Path::new(name)
    .extension()
    .is_some_and(|ext| ext.eq_ignore_ascii_case("yml") || ext.eq_ignore_ascii_case("yaml"))
}

pub(crate) fn bun_binary() -> Result<PathBuf, String> {
  if let Some(path) = std::env::var_os("TOOLU_BUN") {
    return Ok(PathBuf::from(path));
  }
  if Command::new("bun")
    .arg("--version")
    .output()
    .is_ok_and(|output| output.status.success())
  {
    return Ok(PathBuf::from("bun"));
  }
  if let Some(home) = std::env::var_os("HOME") {
    let candidate = PathBuf::from(home).join(".bun/bin/bun");
    if candidate.is_file() {
      return Ok(candidate);
    }
  }
  Err("bun is not installed".to_owned())
}

#[cfg(test)]
#[path = "tests/ci_yaml_test.rs"]
mod tests;
