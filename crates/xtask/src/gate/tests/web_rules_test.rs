use std::path::{Path, PathBuf};
use std::process::Command;

use super::run;
use crate::Verdict;
use crate::options::Options;

fn root() -> PathBuf {
  PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
    .canonicalize()
    .unwrap()
}

fn output(program: &str, args: &[&str]) -> (bool, String) {
  let result = Command::new(program)
    .args(args)
    .current_dir(root())
    .output()
    .unwrap();
  let text = format!(
    "{}{}",
    String::from_utf8_lossy(&result.stdout),
    String::from_utf8_lossy(&result.stderr)
  );
  (result.status.success(), text)
}

fn assert_ast_case(base: &Path, patterns: &Path, file: &str, rule: &str, line: usize) {
  let rule_file = patterns.join(format!("{rule}.yml"));
  let target = base.join(file);
  let (ok, text) = output(
    "ast-grep",
    &[
      "scan",
      "--rule",
      &rule_file.to_string_lossy(),
      "--report-style",
      "short",
      &target.to_string_lossy(),
    ],
  );
  assert!(!ok, "{rule} failed to fire on {file}: {text}");
  assert!(text.contains(rule), "{rule}: {text}");
  assert!(text.contains(file), "{rule}: {text}");
  assert!(text.contains(&format!(":{line}:")), "{rule}: {text}");
  assert_eq!(text.matches(rule).count(), 1, "{rule}: {text}");
}

#[test]
fn web_gate_rules_ast_patterns_name_one_violation_and_location() {
  let root = root();
  let base = root.join("fixtures/web-gate/violating");
  let patterns = root.join("tooling/conventions/guardrails/web/patterns");
  let cases = [
    ("board.css", "no-hardcoded-color-css", 2),
    ("rgb.css", "no-hardcoded-color-css", 2),
    ("hsl.css", "no-hardcoded-color-css", 2),
    ("board.css", "no-external-url-css", 5),
    ("external-unquoted.css", "no-external-url-css", 2),
    ("external-uppercase.css", "no-external-url-css", 1),
    ("external-import.css", "no-external-url-css", 1),
    ("js/view/color.js", "no-hardcoded-color-js", 1),
    ("js/view/rgb.js", "no-hardcoded-color-js", 1),
    ("js/view/hsl.js", "no-hardcoded-color-js", 1),
    ("js/view/external.js", "no-external-url-js", 1),
    ("js/view/external-uppercase.js", "no-external-url-js", 1),
    ("js/view/fetch.js", "no-bare-fetch", 2),
    ("js/view/window-fetch.js", "no-bare-fetch", 1),
    ("js/view/self-fetch.js", "no-bare-fetch", 1),
    ("js/view/global-fetch.js", "no-bare-fetch", 1),
    ("js/logic/dom.js", "no-dom-in-logic", 2),
    ("js/view/suppression.js", "no-suppression-js", 1),
    ("js/view/eslint-suppression.js", "no-suppression-js", 1),
    ("index.html", "no-external-url-html", 6),
    ("external-uppercase.html", "no-external-url-html", 1),
    ("index.html", "no-inline-script", 7),
    ("inline-module.html", "no-inline-script", 1),
    ("index.html", "no-inline-style", 8),
    ("inline-style-media.html", "no-inline-style", 1),
    ("index.html", "no-inline-attribute", 11),
    ("inline-style-attribute.html", "no-inline-attribute", 1),
    ("uppercase-handler.html", "no-inline-attribute", 1),
    ("index.html", "no-suppression-html", 12),
    ("suppression.css", "no-suppression-css", 1),
  ];
  for (file, rule, line) in cases {
    assert_ast_case(&base, &patterns, file, rule, line);
  }
}

#[test]
fn web_gate_rules_oxlint_security_bans_name_file_and_line() {
  let root = root();
  let config = root.join("tooling/conventions/guardrails/web/oxlintrc.json");
  let base = root.join("fixtures/web-gate/violating/oxlint");
  let cases = [
    ("inner-html.js", "no-restricted-properties"),
    ("outer-html.js", "no-restricted-properties"),
    ("adjacent-html.js", "no-restricted-properties"),
    ("document-write.js", "no-restricted-properties"),
    ("document-writeln.js", "no-restricted-properties"),
    ("eval.js", "no-eval"),
    ("new-function.js", "no-new-func"),
    ("implied-eval.js", "no-implied-eval"),
    ("local-storage.js", "no-restricted-globals"),
    ("window-storage.js", "no-restricted-properties"),
    ("script-url.js", "no-script-url"),
  ];
  for (file, rule) in cases {
    let target = base.join(file);
    let (ok, text) = output(
      &root.join("node_modules/.bin/oxlint").to_string_lossy(),
      &[
        "--config",
        &config.to_string_lossy(),
        "--format",
        "unix",
        &target.to_string_lossy(),
      ],
    );
    assert!(!ok, "{rule} failed to fire on {file}: {text}");
    assert!(text.contains(rule), "{file}: {text}");
    assert!(text.contains(file), "{file}: {text}");
    assert!(text.contains(":1:"), "{file}: {text}");
  }
}

#[test]
fn web_gate_rules_browser_modules_reject_node_globals() {
  let root = root();
  let config = root.join("tooling/conventions/guardrails/web/oxlintrc.json");
  let file = root.join("fixtures/web-gate/violating/oxlint/node-global.js");
  let (ok, text) = output(
    &root.join("node_modules/.bin/oxlint").to_string_lossy(),
    &[
      "--config",
      &config.to_string_lossy(),
      "--format",
      "unix",
      &file.to_string_lossy(),
    ],
  );
  assert!(
    !ok && text.contains("no-undef") && text.contains("node-global.js:1:"),
    "{text}"
  );
}

#[test]
fn web_gate_rules_format_and_accessibility_fail() {
  let root = root();
  let bad = root.join("fixtures/web-gate/violating");
  let config = root.join("tooling/conventions/guardrails/web");
  let (ok, formatted) = output(
    &root.join("node_modules/.bin/oxfmt").to_string_lossy(),
    &[
      "--check",
      "--config",
      &config.join("oxfmtrc.json").to_string_lossy(),
      &bad.join("format.css").to_string_lossy(),
    ],
  );
  assert!(!ok && formatted.contains("format.css"), "{formatted}");
  let (ok, html) = output(
    &root
      .join("node_modules/.bin/html-validate")
      .to_string_lossy(),
    &[
      "--config",
      &config.join("html-validate.json").to_string_lossy(),
      &bad.join("a11y.html").to_string_lossy(),
    ],
  );
  assert!(
    !ok && html.contains("wcag/h37") && html.contains("8:"),
    "{html}"
  );
}

#[test]
fn web_gate_rules_jscpd_finds_duplicate_ten_line_block() {
  let root = root();
  let config = root.join("tooling/conventions/guardrails/web/jscpd.json");
  let dir = root.join("fixtures/web-gate/violating/duplicates");
  let (ok, text) = output(
    &root.join("node_modules/.bin/jscpd").to_string_lossy(),
    &[
      "--config",
      &config.to_string_lossy(),
      &dir.to_string_lossy(),
    ],
  );
  assert!(
    !ok && text.contains("one.js") && text.contains("two.js"),
    "{text}"
  );
}

#[test]
fn web_gate_rules_violating_fixture_fails_the_full_step() {
  let options = Options {
    root: root(),
    files: vec![PathBuf::from("fixtures/web-gate/violating")],
    only: vec!["web".to_owned()],
    ..Options::default()
  };
  assert_eq!(run(&options).unwrap(), Verdict::Findings);
}
