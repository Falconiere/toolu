//! Shared run-case parity and cross-language state in real Git worktrees.

#[cfg(test)]
#[path = "helpers/quality_run_gate.rs"]
mod quality_run_gate;

#[cfg(test)]
mod tests {
  use std::path::{Path, PathBuf};
  use std::process::Command;

  use serde_json::{Map, Value, json};
  use toolu_engine::quality::{
    AstGrepScan, EditedFile, QualityFindings, QualityOutcome, QualityRule, run_quality,
    run_quality_event,
  };
  use toolu_protocol::decision::merge;
  use toolu_protocol::host::Host;
  use toolu_protocol::normalized::NormalizedEvent;
  use toolu_runtime::env::Env;
  use toolu_runtime::registry::rule::RuleContext;
  use toolu_state::edit_records::{EditRecords, normalize_edit_records};

  use super::quality_run_gate::gate_entries;

  struct Project {
    _temp: tempfile::TempDir,
    root: PathBuf,
    repo: PathBuf,
  }

  impl Project {
    fn new() -> Self {
      let temp = tempfile::tempdir().unwrap();
      let root = std::fs::canonicalize(temp.path()).unwrap();
      let repo = root.join("project");
      std::fs::create_dir_all(&repo).unwrap();
      let project = Self {
        _temp: temp,
        root,
        repo,
      };
      project.git(&["init", "-q"]);
      project.git(&[
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "commit",
        "-q",
        "--allow-empty",
        "-m",
        "init",
      ]);
      project
    }

    fn git(&self, args: &[&str]) {
      let output = Command::new("git")
        .args(args)
        .current_dir(&self.repo)
        .output()
        .unwrap();
      assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
      );
    }

    fn write(&self, path: &str, body: &str) {
      let file = self.repo.join(path);
      std::fs::create_dir_all(file.parent().unwrap()).unwrap();
      std::fs::write(file, body).unwrap();
    }

    fn gate(root: &Path) -> PathBuf {
      root.join(".claude/tmp/quality-gate-status.json")
    }

    fn env(&self) -> Env {
      Env::process().with("HOME", self.root.join("home").to_string_lossy().as_ref())
    }

    fn context<'a>(&'a self, env: &'a Env, raw: &'a Map<String, Value>) -> RuleContext<'a> {
      RuleContext {
        host: Host::Claude,
        env,
        config_root: &self.root,
        project_root: &self.repo,
        cwd: Some(&self.repo),
        plugin_root: None,
        raw,
        edit: None,
      }
    }
  }

  struct Rule {
    ext: &'static str,
    source: &'static str,
    skip_linked: bool,
    findings: QualityFindings,
  }

  impl Rule {
    fn new(
      ext: &'static str,
      source: &'static str,
      errors: Vec<String>,
      advisories: Vec<String>,
    ) -> Self {
      Self {
        ext,
        source,
        skip_linked: false,
        findings: QualityFindings { errors, advisories },
      }
    }
  }

  impl QualityRule for Rule {
    fn language(&self) -> &str {
      self.ext
    }
    fn extensions(&self) -> &[&str] {
      std::slice::from_ref(&self.ext)
    }
    fn source(&self) -> &str {
      self.source
    }
    fn reason(&self) -> &'static str {
      "fixture failure"
    }
    fn project_enabled(&self, _ctx: &RuleContext<'_>) -> bool {
      true
    }
    fn skip_linked_worktrees(&self) -> bool {
      self.skip_linked
    }
    fn ast_rule_dirs(&self) -> Vec<PathBuf> {
      Vec::new()
    }
    fn check(
      &self,
      _file: &EditedFile,
      _ctx: &RuleContext<'_>,
      _scan: &AstGrepScan,
    ) -> QualityFindings {
      self.findings.clone()
    }
  }

  fn event(call: &Value, repo: &Path) -> NormalizedEvent {
    serde_json::from_value(json!({
      "type":"tool/post", "sessionId":"s", "cwd":repo,
      "projectRoot":repo, "worktree":repo, "toolCallId":"t",
      "toolName":call["toolName"].as_str().unwrap_or("Write"),
      "toolInput":call["input"].as_object().cloned().unwrap_or_default()
    }))
    .unwrap()
  }

  fn strings(value: &Value) -> Vec<String> {
    value.as_array().map_or_else(Vec::new, |items| {
      items
        .iter()
        .map(|item| item.as_str().unwrap().to_owned())
        .collect()
    })
  }

  fn check_run_step(project: &Project, name: &Value, step: &Value) {
    let rule = Rule::new(
      "fx",
      "fixture-quality",
      strings(&step["errors"]),
      strings(&step["advisories"]),
    );
    let env = project.env();
    let raw = Map::new();
    let outcome: QualityOutcome = run_quality_event(
      &event(&step["call"], &project.repo),
      &project.context(&env, &raw),
      &[&rule],
    );
    assert!(
      outcome.warnings.is_empty(),
      "{name}: {:?}",
      outcome.warnings
    );
    if !step["decision"].is_null() {
      assert_eq!(
        serde_json::to_value(merge(&outcome.decisions)).unwrap(),
        step["decision"],
        "{name}"
      );
    }
    let gate = Project::gate(&project.repo);
    if !step["entries"].is_null() {
      assert_eq!(gate_entries(&gate), step["entries"], "{name}");
    }
    if let Some(status) = step["gateStatus"].as_str() {
      let observed = if gate.exists() {
        serde_json::from_slice::<Value>(&std::fs::read(&gate).unwrap()).unwrap()["status"]
          .as_str()
          .unwrap()
          .to_owned()
      } else {
        "missing".to_owned()
      };
      assert_eq!(observed, status, "{name}");
    }
  }

  #[test]
  fn quality_run_replays_every_shared_run_case() {
    let fixture: Value = serde_json::from_slice(
      &std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/quality/runner.json"),
      )
      .unwrap(),
    )
    .unwrap();
    let mut checked = 0;
    for case in fixture["cases"]
      .as_array()
      .unwrap()
      .iter()
      .filter(|case| case["kind"] == "run")
    {
      let project = Project::new();
      for (path, body) in case["files"].as_object().unwrap() {
        project.write(path, body.as_str().unwrap());
      }
      for step in case["steps"].as_array().unwrap() {
        check_run_step(&project, &case["name"], step);
        checked += 1;
      }
    }
    assert!(checked >= 10, "run cases were not loaded");
  }

  #[test]
  fn quality_move_from_ts_to_py_clears_old_owner_and_records_new_owner() {
    let project = Project::new();
    project.write("a.ts", "bad\n");
    let env = project.env();
    let raw = Map::new();
    let ctx = project.context(&env, &raw);
    let ts = Rule::new("ts", "ts-quality", vec!["old error".to_owned()], Vec::new());
    let initial = event(&json!({"input":{"file_path":"a.ts"}}), &project.repo);
    assert_eq!(run_quality_event(&initial, &ctx, &[&ts]).decisions.len(), 1);
    std::fs::rename(project.repo.join("a.ts"), project.repo.join("a.py")).unwrap();
    let patch =
      "*** Begin Patch\n*** Update File: a.ts\n*** Move to: a.py\n@@\n-bad\n+bad\n*** End Patch";
    let EditRecords::Records(records) =
      normalize_edit_records(&json!({"tool_input":{"command":patch}}), "apply_patch")
    else {
      panic!("move patch did not parse");
    };
    let py = Rule::new(
      "py",
      "python-quality",
      vec!["new error".to_owned()],
      Vec::new(),
    );
    let result = run_quality(&records, &ctx, &[&ts, &py]);
    assert_eq!(result.decisions.len(), 1);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    let entries = gate_entries(&Project::gate(&project.repo));
    assert_eq!(entries, json!({"a.py":"new error\n"}));
  }

  #[test]
  fn quality_linked_worktree_records_python_in_its_own_state_and_skips_ts() {
    let project = Project::new();
    let linked = project.root.join("linked");
    project.git(&[
      "worktree",
      "add",
      "-q",
      "-b",
      "side",
      linked.to_str().unwrap(),
    ]);
    std::fs::write(linked.join("a.py"), "bad\n").unwrap();
    std::fs::write(linked.join("a.ts"), "bad\n").unwrap();
    let env = project.env();
    let raw = Map::new();
    let ctx = project.context(&env, &raw);
    let mut ts = Rule::new("ts", "ts-quality", vec!["ts error".to_owned()], Vec::new());
    ts.skip_linked = true;
    let py = Rule::new(
      "py",
      "python-quality",
      vec!["py error".to_owned()],
      Vec::new(),
    );
    let patch = "*** Begin Patch\n*** Update File: ../linked/a.py\n@@\n-bad\n+bad\n*** Update File: ../linked/a.ts\n@@\n-bad\n+bad\n*** End Patch";
    let EditRecords::Records(records) =
      normalize_edit_records(&json!({"tool_input":{"command":patch}}), "apply_patch")
    else {
      panic!("linked patch did not parse");
    };
    let result = run_quality(&records, &ctx, &[&ts, &py]);
    assert_eq!(result.decisions.len(), 1);
    assert!(result.warnings.is_empty(), "{:?}", result.warnings);
    assert_eq!(
      gate_entries(&Project::gate(&linked)),
      json!({"../linked/a.py":"py error\n"})
    );
    assert!(!Project::gate(&project.repo).exists());
  }
}
