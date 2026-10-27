use std::path::PathBuf;
use std::process::Command;

use super::{check, code_lines};

fn root() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    .canonicalize()
    .unwrap()
}

#[test]
fn web_gate_limits_code_lines_exclude_blank_and_comments() {
  assert_eq!(code_lines("a {}\n\n/* comment */\nb {}\n", "css"), 2);
  assert_eq!(
    code_lines("<main>x</main>\n<!-- comment\ncontinued -->\n", "html"),
    1
  );
  assert_eq!(code_lines(&"a {}\n".repeat(300), "css"), 300);
  assert_eq!(code_lines(&"a {}\n".repeat(301), "css"), 301);
}

#[test]
fn web_gate_limits_html_css_code_lines() {
  let dir = tempfile::tempdir().unwrap();
  let asset = dir.path().join("dashboard");
  std::fs::create_dir_all(asset.join("fonts")).unwrap();
  std::fs::write(asset.join("index.html"), "<main></main>").unwrap();
  std::fs::write(asset.join("board.css"), "a {}\n".repeat(300)).unwrap();
  let found = check(&root(), &asset).unwrap();
  assert!(found.is_empty(), "{found:?}");
  std::fs::write(asset.join("board.css"), "a {}\n".repeat(301)).unwrap();
  assert!(
    check(&root(), &asset)
      .unwrap()
      .iter()
      .any(|x| x.contains("max-code-lines"))
  );
  std::fs::write(asset.join("board.css"), "").unwrap();
  std::fs::write(asset.join("index.html"), "<p>x</p>\n".repeat(301)).unwrap();
  assert!(
    check(&root(), &asset)
      .unwrap()
      .iter()
      .any(|x| x.contains("max-code-lines"))
  );
}

#[test]
fn web_gate_limits_served_and_font_bytes() {
  let dir = tempfile::tempdir().unwrap();
  let asset = dir.path().join("dashboard");
  std::fs::create_dir_all(asset.join("fonts")).unwrap();
  std::fs::write(asset.join("index.html"), "<main></main>").unwrap();
  let head = std::fs::read(asset.join("index.html")).unwrap().len();
  std::fs::write(
    asset.join("board.css"),
    format!("/*{}*/", "x".repeat(150_000 - head - 4)),
  )
  .unwrap();
  assert_eq!(check(&root(), &asset).unwrap(), Vec::<String>::new());
  std::fs::write(
    asset.join("board.css"),
    format!("/*{}*/", "x".repeat(150_001 - head - 4)),
  )
  .unwrap();
  assert!(
    check(&root(), &asset)
      .unwrap()
      .iter()
      .any(|x| x.contains("max-served-bytes"))
  );
  std::fs::write(asset.join("board.css"), "").unwrap();
  std::fs::write(asset.join("fonts/font.woff2"), vec![0_u8; 200_000]).unwrap();
  std::fs::create_dir_all(asset.join("tests")).unwrap();
  std::fs::write(asset.join("tests/large.test.js"), vec![b' '; 200_001]).unwrap();
  std::fs::write(asset.join("fonts/OFL.txt"), vec![b' '; 200_001]).unwrap();
  assert_eq!(check(&root(), &asset).unwrap(), Vec::<String>::new());
  std::fs::write(asset.join("fonts/font.woff2"), vec![0_u8; 200_001]).unwrap();
  assert!(
    check(&root(), &asset)
      .unwrap()
      .iter()
      .any(|x| x.contains("max-font-bytes"))
  );
}

#[cfg(unix)]
#[test]
fn web_gate_tests_symlink_cannot_escape_the_asset_tree() {
  let dir = tempfile::tempdir().unwrap();
  let asset = dir.path().join("dashboard");
  std::fs::create_dir_all(&asset).unwrap();
  std::os::unix::fs::symlink("../outside", asset.join("linked.css")).unwrap();
  let err = check(&root(), &asset).unwrap_err();
  assert!(err.contains("symlink in asset directory"), "{err}");
}

#[test]
fn web_gate_tests_logic_module_requires_matching_node_test() {
  let dir = tempfile::tempdir().unwrap();
  let asset = dir.path().join("dashboard");
  std::fs::create_dir_all(asset.join("js/logic")).unwrap();
  std::fs::create_dir_all(asset.join("tests")).unwrap();
  std::fs::write(asset.join("js/logic/state.js"), "export const state = 1;\n").unwrap();
  assert!(
    check(&root(), &asset)
      .unwrap()
      .iter()
      .any(|x| x.contains("logic-test-pair"))
  );
  std::fs::write(asset.join("tests/state.test.js"), "import 'node:test';\n").unwrap();
  assert_eq!(check(&root(), &asset).unwrap(), Vec::<String>::new());
}

#[test]
fn web_gate_tests_node_propagates_a_failing_assertion() {
  let file = root().join("fixtures/web-gate/violating/tests/failing.test.js");
  let result = Command::new("node")
    .args(["--test", &file.to_string_lossy()])
    .output()
    .unwrap();
  let text = String::from_utf8_lossy(&result.stdout);
  assert!(
    !result.status.success() && text.contains("ERR_ASSERTION"),
    "{text}"
  );
}

#[test]
fn web_gate_limits_oxlint_enforces_js_file_and_function_lines() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("lines.js");
  let root = root();
  let config = root.join("tooling/conventions/guardrails/web/oxlintrc.json");
  let lint = |source: &str| {
    std::fs::write(&file, source).unwrap();
    let result = Command::new(root.join("node_modules/.bin/oxlint"))
      .args([
        "--config",
        &config.to_string_lossy(),
        &file.to_string_lossy(),
      ])
      .output()
      .unwrap();
    format!(
      "{}{}",
      String::from_utf8_lossy(&result.stdout),
      String::from_utf8_lossy(&result.stderr)
    )
  };
  let lines = |count: usize| {
    use std::fmt::Write;
    let mut source = String::new();
    for i in 0..count {
      writeln!(source, "export const v{i} = {i};").unwrap();
    }
    source
  };
  assert!(!lint(&lines(300)).contains("max-lines"));
  assert!(lint(&lines(301)).contains("max-lines"));
  let function = |items: usize| {
    format!(
      "export function values() {{\n  return [\n{}  ];\n}}\n",
      "    1,\n".repeat(items)
    )
  };
  assert!(!lint(&function(46)).contains("max-lines-per-function"));
  assert!(lint(&function(47)).contains("max-lines-per-function"));
}
