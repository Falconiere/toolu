use std::path::Path;

use serde_json::Map;
use toolu_engine::quality::{AstGrepScan, EditedFile, ScanFailure, ScanStage};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::registry::rule::RuleContext;

use super::{mock_findings, no_mocks};
use crate::file::{Limits, PyFile};

/// The real `ast-grep scan` of `text` as `test_m.py` in a fresh directory.
fn scan_test_file(text: &str, limits: Limits, is_test: bool) -> Vec<String> {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  std::fs::write(root.join("test_m.py"), text).unwrap();
  let file = EditedFile {
    path: "test_m.py".to_owned(),
    absolute: root.join("test_m.py"),
    removed: false,
  };
  let (env, raw) = (Env::process(), Map::new());
  let ctx = RuleContext {
    host: Host::Claude,
    env: &env,
    config_root: root,
    project_root: root,
    cwd: Some(root),
    plugin_root: None,
    raw: &raw,
    edit: None,
  };
  no_mocks(&PyFile::new(&file, text, limits), &ctx, is_test)
}

#[test]
fn mock_imports_and_fixture_parameters_are_found_by_a_real_scan() {
  let text = "import unittest.mock\nfrom mock import Mock\n\n\ndef test_a(mocker):\n    pass\n\n\n\
    def test_b(\n    monkeypatch,\n):\n    pass\n";
  let errors = scan_test_file(text, Limits::default(), true);
  assert_eq!(
    errors,
    [
      "no-mocks: mock import in test_m.py — write against real data/fixtures instead\n1: import \
       unittest.mock\n2: from mock import Mock",
      "no-mocks: mocker/monkeypatch fixture parameter in test_m.py — write against real \
       data/fixtures instead\n5: def test_a(mocker):\n9: def test_b("
    ]
  );
  let off = Limits {
    no_mocks: false,
    ..Limits::default()
  };
  assert_eq!(scan_test_file(text, off, true).len(), 0);
  assert_eq!(scan_test_file(text, Limits::default(), false).len(), 0);
  let real = "import os\n\n\ndef test_a(tmp_path):\n    pass\n";
  assert_eq!(scan_test_file(real, Limits::default(), true).len(), 0);
}

fn judged(scan: &AstGrepScan) -> Vec<String> {
  let file = EditedFile {
    path: "test_x.py".to_owned(),
    absolute: Path::new("/nonexistent/test_x.py").to_path_buf(),
    removed: false,
  };
  mock_findings(&PyFile::new(&file, "", Limits::default()), scan)
}

const BROKEN: &str = "ast-grep failed while scanning test_x.py — ";
const UNVERIFIED: &str = "; no-mocks rules could not be verified. Fix the tool/file and re-edit";

#[test]
fn a_missing_tool_is_skipped_and_a_broken_scan_is_a_violation() {
  assert_eq!(judged(&AstGrepScan::Missing).len(), 0);
  let blank = AstGrepScan::Ok {
    hits: Vec::new(),
    empty: true,
  };
  assert_eq!(
    judged(&blank),
    [format!(
      "{BROKEN}ast-grep exited 0 with empty output (expected at least the JSON array \"[]\"){UNVERIFIED}"
    )]
  );
  let failure = |stage, stderr: &str| {
    AstGrepScan::Failed(ScanFailure {
      stage,
      exit_code: 2,
      stderr_first: stderr.to_owned(),
    })
  };
  assert_eq!(
    judged(&failure(ScanStage::AstGrep, "boom")),
    [format!("{BROKEN}ast-grep exit 2: boom{UNVERIFIED}")]
  );
  assert_eq!(
    judged(&failure(ScanStage::AstGrep, "")),
    [format!("{BROKEN}ast-grep exit 2{UNVERIFIED}")]
  );
  assert_eq!(
    judged(&failure(ScanStage::Parse, "")),
    [format!(
      "{BROKEN}ast-grep exited 0 but its output did not parse as the documented JSON array{UNVERIFIED}"
    )]
  );
}
