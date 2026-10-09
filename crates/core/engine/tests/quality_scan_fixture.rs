//! Structural scan parity from the exported quality-runner JSON cases.

#[cfg(test)]
mod tests {

  use std::os::unix::fs::PermissionsExt as _;
  use std::path::{Path, PathBuf};
  use std::process::Command;

  use serde_json::{Map, Value, json};
  use toolu_engine::quality::{AstGrepScan, EditedFile, ScanStage, scan_inline, scan_rule_dirs};
  use toolu_runtime::env::Env;
  use toolu_runtime::registry::rule::RuleContext;

  fn fixture() -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures/quality/runner.json");
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
  }

  fn context<'a>(root: &'a Path, env: &'a Env, raw: &'a Map<String, Value>) -> RuleContext<'a> {
    RuleContext {
      host: toolu_protocol::host::Host::Claude,
      env,
      config_root: root,
      project_root: root,
      cwd: Some(root),
      plugin_root: None,
      raw,
      edit: None,
    }
  }

  fn scanned_json(scan: &AstGrepScan) -> Value {
    match scan {
      AstGrepScan::Missing => json!({"kind":"missing"}),
      AstGrepScan::Failed(failure) => json!({
        "kind":"failed", "stage": if failure.stage == ScanStage::Parse {"parse"} else {"ast-grep"},
        "exitCode":failure.exit_code, "stderrFirst":failure.stderr_first
      }),
      AstGrepScan::Ok { hits, empty } => json!({
        "kind":"ok", "empty":empty,
        "hits":hits.iter().map(|hit| json!({
          "ruleId":hit.rule_id, "line":hit.line, "excerpt":hit.excerpt,
          "text":hit.text, "first":hit.first
        })).collect::<Vec<_>>()
      }),
    }
  }

  fn stub_env(root: &Path, spec: &Value) -> Env {
    let mut env = Env::process();
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    match spec["kind"].as_str() {
      Some("pathWithoutAstGrep") => env = env.with("PATH", bin.to_str().unwrap()),
      Some("stubAstGrep") => {
        let script = bin.join("ast-grep");
        std::fs::write(
          &script,
          format!("#!/bin/sh\n{}\n", spec["body"].as_str().unwrap()),
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        env = env.with("PATH", bin.to_str().unwrap());
      }
      _ => {}
    }
    env
  }

  fn assert_case(check: &Value, scan: &AstGrepScan) {
    let expected = &check["expect"];
    match expected["kind"].as_str().unwrap() {
      "exact" => assert_eq!(scanned_json(scan), expected["result"]),
      "rules" => assert_rule_case(expected, scan),
      "lines" => assert_line_case(expected, scan),
      "failure" => assert_failure_case(expected, scan),
      kind => panic!("unknown expectation {kind}"),
    }
  }

  fn assert_rule_case(expected: &Value, scan: &AstGrepScan) {
    let AstGrepScan::Ok { hits, empty } = scan else {
      panic!("{scan:?}")
    };
    assert_eq!(*empty, expected["empty"].as_bool().unwrap());
    assert_eq!(
      hits.len(),
      usize::try_from(expected["count"].as_u64().unwrap()).unwrap()
    );
    for (rule, expected_hits) in expected["hitsByRule"].as_object().unwrap() {
      let actual: Vec<Value> = hits
        .iter()
        .filter(|hit| &hit.rule_id == rule)
        .map(|hit| {
          json!({
            "ruleId":hit.rule_id, "line":hit.line, "excerpt":hit.excerpt,
            "text":hit.text, "first":hit.first
          })
        })
        .collect();
      assert_eq!(Value::Array(actual), *expected_hits);
    }
  }

  fn assert_line_case(expected: &Value, scan: &AstGrepScan) {
    let AstGrepScan::Ok { hits, .. } = scan else {
      panic!("{scan:?}")
    };
    let actual: Vec<Value> = hits
      .iter()
      .map(|hit| {
        json!({
          "line":hit.line, "text":hit.text, "first":hit.first
        })
      })
      .collect();
    assert_eq!(Value::Array(actual), expected["lines"]);
  }

  fn assert_failure_case(expected: &Value, scan: &AstGrepScan) {
    let AstGrepScan::Failed(failure) = scan else {
      panic!("{scan:?}")
    };
    let stage = if failure.stage == ScanStage::Parse {
      "parse"
    } else {
      "ast-grep"
    };
    assert_eq!(stage, expected["stage"].as_str().unwrap());
    if expected["nonzeroExit"] == true {
      assert_ne!(failure.exit_code, 0);
    }
    let count = failure.stderr_first.chars().count() as u64;
    assert!(count >= expected["stderrMin"].as_u64().unwrap());
    assert!(count <= expected["stderrMax"].as_u64().unwrap());
  }

  #[test]
  fn quality_scan_replays_every_exported_scan_case() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().unwrap();
    let mut count = 0;
    for case in cases.iter().filter(|case| case["kind"] == "scan") {
      for check in case["checks"].as_array().unwrap() {
        let temp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(temp.path()).unwrap();
        let rel = check["path"].as_str().unwrap();
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, check["body"].as_str().unwrap()).unwrap();
        let env = stub_env(&root, &check["env"]);
        let raw = Map::new();
        let file = EditedFile {
          path: rel.to_owned(),
          absolute: path,
          removed: false,
        };
        let scan = scan_inline(
          &[file],
          check["rules"].as_str().unwrap(),
          &context(&root, &env, &raw),
        );
        assert_case(check, &scan);
        count += 1;
      }
    }
    assert!(count >= 6, "scan fixtures did not load");
  }

  fn real_ast_grep() -> PathBuf {
    let out = Command::new("sh")
      .args(["-c", "command -v ast-grep"])
      .output()
      .unwrap();
    assert!(out.status.success());
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
  }

  fn batch_inputs(root: &Path) -> ([EditedFile; 2], PathBuf, PathBuf) {
    let ts = root.join("a.ts");
    let py = root.join("b.py");
    std::fs::write(&ts, "throw \"boom\";\n").unwrap();
    std::fs::write(&py, "def f(mocker):\n  return 1\n").unwrap();
    let ts_dir = root.join("ts-rules");
    let py_dir = root.join("py-rules");
    std::fs::create_dir_all(&ts_dir).unwrap();
    std::fs::create_dir_all(&py_dir).unwrap();
    std::fs::write(ts_dir.join("throw.yml"), "id: throw-string\nlanguage: ts\nseverity: warning\nmessage: x\nrule:\n  pattern: 'throw \"$S\"'\n").unwrap();
    std::fs::write(py_dir.join("mocker.yml"), "id: mocker-param\nlanguage: python\nseverity: warning\nmessage: m\nrule:\n  kind: function_definition\n  has:\n    field: parameters\n    has: {kind: identifier, regex: '^mocker$'}\n").unwrap();
    let files = [
      EditedFile {
        path: "a.ts".to_owned(),
        absolute: ts,
        removed: false,
      },
      EditedFile {
        path: "b.py".to_owned(),
        absolute: py,
        removed: false,
      },
    ];
    (files, ts_dir, py_dir)
  }

  fn counting_env(root: &Path) -> (Env, PathBuf) {
    let bin = root.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let count = root.join("spawns");
    let script = bin.join("ast-grep");
    std::fs::write(
      &script,
      format!(
        "#!/bin/sh\nprintf 'spawn\\n' >> '{}'\nexec '{}' \"$@\"\n",
        count.display(),
        real_ast_grep().display()
      ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path_text = format!(
      "{}:{}",
      bin.display(),
      Env::process().get("PATH").unwrap_or("")
    );
    let env = Env::process().with("PATH", &path_text);
    (env, count)
  }

  #[test]
  fn quality_scan_batch_spawns_once_and_routes_both_languages() {
    let temp = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(temp.path()).unwrap();
    let (files, ts_dir, py_dir) = batch_inputs(&root);
    let (env, count) = counting_env(&root);
    let raw = Map::new();
    let scan = scan_rule_dirs(&files, &[&ts_dir, &py_dir], &context(&root, &env, &raw));
    let AstGrepScan::Ok { hits, .. } = scan else {
      panic!("{scan:?}")
    };
    assert!(
      hits
        .iter()
        .any(|hit| hit.file == "a.ts" && hit.rule_id == "throw-string")
    );
    assert!(
      hits
        .iter()
        .any(|hit| hit.file == "b.py" && hit.rule_id == "mocker-param")
    );
    assert_eq!(std::fs::read_to_string(&count).unwrap().lines().count(), 1);
    let no_files = scan_rule_dirs(&[], &[&ts_dir, &py_dir], &context(&root, &env, &raw));
    assert!(matches!(no_files, AstGrepScan::Ok { .. }));
    assert_eq!(std::fs::read_to_string(&count).unwrap().lines().count(), 1);
  }
}
