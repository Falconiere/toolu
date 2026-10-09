//! Two language rules share one real structural scan through the quality runner.

#[cfg(test)]
mod tests {
  use std::os::unix::fs::PermissionsExt as _;
  use std::path::{Path, PathBuf};
  use std::process::Command;

  use serde_json::Map;
  use toolu_engine::quality::{AstGrepScan, EditedFile, QualityFindings, QualityRule, run_quality};
  use toolu_protocol::host::Host;
  use toolu_runtime::env::Env;
  use toolu_runtime::registry::rule::RuleContext;
  use toolu_state::edit_records::{EditOperation, EditRecord};
  use toolu_state::gate_file::{GateRead, read_gate_file};
  use toolu_state::gate_schema::GateFile;

  struct ScanRule {
    ext: &'static str,
    id: &'static str,
    dir: PathBuf,
  }

  impl QualityRule for ScanRule {
    fn language(&self) -> &str {
      self.ext
    }
    fn extensions(&self) -> &[&str] {
      std::slice::from_ref(&self.ext)
    }
    fn source(&self) -> &str {
      self.id
    }
    fn reason(&self) -> &'static str {
      "structural finding"
    }
    fn project_enabled(&self, _ctx: &RuleContext<'_>) -> bool {
      true
    }
    fn skip_linked_worktrees(&self) -> bool {
      false
    }
    fn ast_rule_dirs(&self) -> Vec<PathBuf> {
      vec![self.dir.clone()]
    }
    fn check(
      &self,
      file: &EditedFile,
      _ctx: &RuleContext<'_>,
      scan: &AstGrepScan,
    ) -> QualityFindings {
      let AstGrepScan::Ok { hits, .. } = scan else {
        panic!("{scan:?}")
      };
      assert!(hits.iter().all(|hit| hit.file == file.path));
      QualityFindings {
        errors: hits
          .iter()
          .filter(|hit| hit.rule_id == self.id)
          .map(|hit| hit.excerpt.clone())
          .collect(),
        advisories: Vec::new(),
      }
    }
  }

  fn real_ast_grep() -> PathBuf {
    let output = Command::new("sh")
      .args(["-c", "command -v ast-grep"])
      .output()
      .unwrap();
    assert!(output.status.success());
    PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
  }

  fn counting_env(root: &Path) -> (Env, PathBuf) {
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let count = root.join("scan-count");
    let wrapper = bin.join("ast-grep");
    std::fs::write(
      &wrapper,
      format!(
        "#!/bin/sh\nprintf 'scan\\n' >> '{}'\nexec '{}' \"$@\"\n",
        count.display(),
        real_ast_grep().display()
      ),
    )
    .unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!(
      "{}:{}",
      bin.display(),
      Env::process().get("PATH").unwrap_or("")
    );
    (Env::process().with("PATH", &path), count)
  }

  fn rules(root: &Path) -> [ScanRule; 2] {
    let ts = root.join("ts-rules");
    let py = root.join("py-rules");
    std::fs::create_dir_all(&ts).unwrap();
    std::fs::create_dir_all(&py).unwrap();
    std::fs::write(ts.join("throw.yml"), "id: throw-string\nlanguage: ts\nseverity: warning\nmessage: x\nrule:\n  pattern: 'throw \"$S\"'\n").unwrap();
    std::fs::write(py.join("mocker.yml"), "id: mocker-param\nlanguage: python\nseverity: warning\nmessage: m\nrule:\n  kind: function_definition\n  has:\n    field: parameters\n    has: {kind: identifier, regex: '^mocker$'}\n").unwrap();
    [
      ScanRule {
        ext: "ts",
        id: "throw-string",
        dir: ts,
      },
      ScanRule {
        ext: "py",
        id: "mocker-param",
        dir: py,
      },
    ]
  }

  #[test]
  fn quality_scan_batch_runs_once_and_settles_each_language() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temp.path()).unwrap();
    std::fs::write(root.join("a.ts"), "throw \"boom\";\n").unwrap();
    std::fs::write(root.join("b.py"), "def f(mocker):\n  return 1\n").unwrap();
    let rules = rules(&root);
    let (env, count) = counting_env(&root);
    let raw = Map::new();
    let ctx = RuleContext {
      host: Host::Claude,
      env: &env,
      config_root: &root,
      project_root: &root,
      cwd: Some(&root),
      plugin_root: None,
      raw: &raw,
      edit: None,
    };
    let records = ["a.ts", "b.py"].map(|path| EditRecord {
      path: path.to_owned(),
      operation: EditOperation::Write,
      moved_to: None,
      from: None,
    });
    let result = run_quality(&records, &ctx, &[&rules[0], &rules[1]]);
    assert_eq!(result.decisions.len(), 2);
    assert_eq!(result.warnings, Vec::<String>::new());
    assert_eq!(std::fs::read_to_string(count).unwrap(), "scan\n");
    let gate = root.join(".claude/tmp/quality-gate-status.json");
    let GateRead::Ok(GateFile::Failing {
      entries: Some(entries),
      ..
    }) = read_gate_file(&gate)
    else {
      panic!("missing quality gate entries");
    };
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].0, "a.ts");
    assert_eq!(entries[0].1.source, "throw-string");
    assert_eq!(entries[0].1.violations, "a.ts:1:throw \"boom\";\n");
    assert_eq!(entries[1].0, "b.py");
    assert_eq!(entries[1].1.source, "mocker-param");
    assert_eq!(
      entries[1].1.violations,
      "b.py:1:def f(mocker):\nb.py:2:  return 1\n"
    );
  }
}
