use std::path::Path;

use toolu_engine::quality::{AstGrepHit, AstGrepScan, EditedFile, ScanFailure, ScanStage};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_state::detect::project::NodePackageManager;

use super::{error_handling, inline_rules, mock_doubles};
use crate::file::{Limits, Place, TsFile};

fn hit(rule: &str, line: i64, source: &str) -> AstGrepHit {
  AstGrepHit {
    rule_id: rule.to_owned(),
    file: "f.ts".to_owned(),
    line,
    excerpt: format!("f.ts:{line}:{source}"),
    text: source.to_owned(),
    first: true,
  }
}

fn scanned(hits: Vec<AstGrepHit>) -> AstGrepScan {
  AstGrepScan::Ok { hits, empty: false }
}

fn failed(stage: ScanStage, stderr_first: &str) -> AstGrepScan {
  AstGrepScan::Failed(ScanFailure {
    stage,
    exit_code: 2,
    stderr_first: stderr_first.to_owned(),
  })
}

/// The two structural rules for `path` holding `text`.
fn structural(path: &str, text: &str, no_mocks: bool, scan: &AstGrepScan) -> Vec<String> {
  let env = Env::default();
  let root = Path::new("/p");
  let file = EditedFile {
    path: path.to_owned(),
    absolute: root.join(path),
    removed: false,
  };
  let at = Place {
    env: &env,
    project_root: root,
    cwd: root,
  };
  let limits = Limits {
    no_mocks,
    ..Limits::default()
  };
  let f = TsFile::new(&file, text, at, limits, NodePackageManager::Bun);
  let mut out = error_handling(&f, scan);
  out.extend(mock_doubles(&f, scan));
  out
}

#[test]
fn opencode_also_scans_tsx_files() {
  let claude = inline_rules(Host::Claude);
  assert!(claude.contains("language: ts\n") && !claude.contains("language: tsx\n"));
  let opencode = inline_rules(Host::Opencode);
  assert!(opencode.starts_with(&claude) && opencode.contains("language: tsx\n"));
}

#[test]
fn error_hits_are_grouped_and_capped_by_rule() {
  let hits = vec![
    hit("empty-catch", 1, "} catch (e) {}\t"),
    hit("swallow-null", 2, "return null;"),
    hit("swallow-null", 3, "return null;"),
    hit("swallow-null", 4, "return null;"),
    hit("throw-string", 5, "throw \"x\";"),
  ];
  assert_eq!(
    structural("src/a.ts", "", true, &scanned(hits)),
    [
      "Empty catch block in src/a.ts — handle the error or rethrow; do not swallow\nf.ts:1:} catch (e) {}",
      "Catch swallows the error by returning a nullish value in src/a.ts — handle, log, or rethrow it\n\
       f.ts:2:return null;\nf.ts:3:return null;",
      "throw of string literal in src/a.ts — throw an Error (or subclass) instead\nf.ts:5:throw \"x\";",
    ]
  );
  assert_eq!(
    structural("src/a.ts", "", true, &AstGrepScan::Missing).len(),
    0
  );
}

#[test]
fn a_broken_scan_is_a_violation_of_both_rules() {
  let crashed = failed(ScanStage::AstGrep, "boom");
  let out = structural("src/a.test.ts", "", true, &crashed);
  assert_eq!(out.len(), 2);
  assert!(out[0].contains("ast-grep failed while scanning src/a.test.ts — ast-grep exit 2: boom;"));
  assert!(out[1].contains("for mocks — exit 2: boom; no-mocks rule could not be verified"));
  let unparsed = structural("src/a.test.ts", "", true, &failed(ScanStage::Parse, ""));
  assert!(unparsed[0].contains("did not parse as the documented JSON array; error-handling"));
  assert!(unparsed[1].contains("for mocks — its output did not parse"));
  let blank = AstGrepScan::Ok {
    hits: Vec::new(),
    empty: true,
  };
  let out = structural("src/a.test.ts", "", true, &blank);
  assert_eq!(out.len(), 1);
  assert!(out[0].contains("exited 0 with empty output"));
  let outside_tests = structural("src/a.ts", "", true, &failed(ScanStage::AstGrep, ""));
  assert_eq!(outside_tests.len(), 1);
}

#[test]
fn mocks_in_tests_are_listed_in_source_order() {
  let hits: Vec<AstGrepHit> = (1..=7)
    .rev()
    .map(|line| hit("vi-fn", line, "vi.fn();"))
    .chain([hit("empty-catch", 9, "x")])
    .collect();
  let scan = scanned(hits);
  let import = "import { mock } from 'ts-mockito';\n";
  let out = structural("src/__tests__/a.ts", import, true, &scan);
  assert_eq!(out.len(), 3);
  assert!(
    out[1]
      .ends_with("vi.fn();\nf.ts:2:vi.fn();\nf.ts:3:vi.fn();\nf.ts:4:vi.fn();\nf.ts:5:vi.fn();")
  );
  assert!(out[1].contains("\nf.ts:1:vi.fn();\n"));
  assert!(out[2].starts_with("Import from ts-mockito in src/__tests__/a.ts"));
  assert_eq!(
    structural("src/__tests__/a.ts", import, false, &scan).len(),
    1
  );
  assert_eq!(
    structural("app/e2e/__tests__/a.ts", import, true, &scan).len(),
    1
  );
  assert_eq!(structural("src/a.ts", import, true, &scan).len(), 1);
}
