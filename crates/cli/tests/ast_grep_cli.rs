//! Real `toolu ast-grep` commands compared with the installed ast-grep binary.

use std::error::Error;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn real_ast_grep() -> Res<PathBuf> {
  let path = std::env::var_os("PATH").ok_or("PATH missing")?;
  std::env::split_paths(&path)
    .map(|dir| dir.join("ast-grep"))
    .find(|path| path.is_file())
    .map(fs::canonicalize)
    .transpose()?
    .ok_or_else(|| "real ast-grep is required".into())
}

fn run(program: &Path, args: &[&str], cwd: &Path, path: &Path) -> Res<Output> {
  Ok(
    Command::new(program)
      .args(args)
      .current_dir(cwd)
      .env("PATH", path)
      .output()?,
  )
}

fn project() -> Res<(tempfile::TempDir, PathBuf)> {
  let dir = tempfile::tempdir()?;
  fs::create_dir(dir.path().join("src"))?;
  fs::write(
    dir.path().join("src/a.ts"),
    "export function greet(name: string) { console.log(name); return name; }\n",
  )?;
  fs::write(
    dir.path().join("src/b.tsx"),
    "export const B = () => { console.log('b'); return <p />; };\n",
  )?;
  fs::write(
    dir.path().join("rule.yml"),
    "id: no-console\nlanguage: typescript\nseverity: warning\nmessage: no console\nrule:\n  pattern: console.log($A)\n",
  )?;
  let bin = dir.path().join("bin");
  fs::create_dir(&bin)?;
  Ok((dir, bin))
}

const CASES: &[(&[&str], &[&str])] = &[
  (
    &["search", "console.log($A)", "src/a.ts"],
    &[
      "run",
      "--pattern",
      "console.log($A)",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/a.ts",
    ],
  ),
  (
    &["files", "console.log($A)", "src/b.tsx"],
    &[
      "run",
      "--pattern",
      "console.log($A)",
      "--files-with-matches",
      "--color",
      "never",
      "--lang",
      "tsx",
      "src/b.tsx",
    ],
  ),
  (
    &["scan", "rule.yml", "src"],
    &[
      "scan",
      "--rule",
      "rule.yml",
      "--report-style",
      "short",
      "--max-results",
      "50",
      "--color",
      "never",
      "src",
    ],
  ),
  (
    &["debug", "console.log($A)", "src/a.ts"],
    &[
      "run",
      "--pattern",
      "console.log($A)",
      "--debug-query=pattern",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/a.ts",
    ],
  ),
];

#[test]
fn search_files_scan_and_debug_match_real_ast_grep() {
  let (dir, bin) = project().expect("project");
  let real = real_ast_grep().expect("installed ast-grep");
  symlink(&real, bin.join("sg")).expect("sg link");
  for (toolu_args, external_args) in CASES {
    let actual = run(
      Path::new(TOOLU),
      &[&["ast-grep"], *toolu_args].concat(),
      dir.path(),
      &bin,
    )
    .expect("toolu wrapper");
    let expected = run(&real, external_args, dir.path(), &bin).expect("external CLI");
    assert_eq!(
      actual.status.code(),
      expected.status.code(),
      "{toolu_args:?}"
    );
    assert_eq!(actual.stdout, expected.stdout, "{toolu_args:?}");
    assert_eq!(actual.stderr, expected.stderr, "{toolu_args:?}");
    assert!(!expected.stdout.is_empty() || !expected.stderr.is_empty());
  }
}

#[test]
fn falls_back_to_ast_grep_when_sg_is_absent() {
  let (dir, bin) = project().expect("project");
  let real = real_ast_grep().expect("installed ast-grep");
  symlink(&real, bin.join("ast-grep")).expect("ast-grep link");
  let actual = run(
    Path::new(TOOLU),
    &["ast-grep", "search", "console.log($A)", "src/a.ts"],
    dir.path(),
    &bin,
  )
  .expect("toolu wrapper");
  let expected = run(
    &real,
    &[
      "run",
      "--pattern",
      "console.log($A)",
      "--color",
      "never",
      "--lang",
      "typescript",
      "src/a.ts",
    ],
    dir.path(),
    &bin,
  )
  .expect("external CLI");
  assert!(actual.status.success());
  assert_eq!(actual.stdout, expected.stdout);
  assert_eq!(actual.stderr, expected.stderr);
}

#[test]
fn missing_external_binary_is_silent_and_json_is_one_document() {
  let (dir, bin) = project().expect("project");
  let output = run(
    Path::new(TOOLU),
    &["ast-grep", "search", "console.log($A)", "src/a.ts"],
    dir.path(),
    &bin,
  )
  .expect("silent wrapper");
  assert!(output.status.success());
  assert!(output.stdout.is_empty() && output.stderr.is_empty());
  let output = run(
    Path::new(TOOLU),
    &[
      "--json",
      "ast-grep",
      "search",
      "console.log($A)",
      "src/a.ts",
    ],
    dir.path(),
    &bin,
  )
  .expect("JSON wrapper");
  let json: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
  assert_eq!(json["exitCode"], 0);
  assert_eq!(json["verb"], "search");
}

#[test]
fn savings_reports_real_jsonl_and_first_invalid_line() {
  let (dir, bin) = project().expect("project");
  let ledger = dir.path().join("session.jsonl");
  fs::write(
    &ledger,
    "{\"kind\":\"read\",\"returned\":120,\"full\":4000}\n{\"kind\":\"ast-grep\",\"returned\":80,\"full\":0}\n",
  )
  .expect("valid ledger");
  let path = ledger.to_str().expect("UTF-8 ledger path");
  let output = run(
    Path::new(TOOLU),
    &["ast-grep", "savings", path],
    dir.path(),
    &bin,
  )
  .expect("savings report");
  assert!(output.status.success());
  let stdout = String::from_utf8(output.stdout).expect("UTF-8 report");
  assert!(stdout.contains("read: returned=120 full=4000 saved=97% (n=1)"));
  assert!(stdout.contains("TOTAL returned: 200 bytes (~50 tok)"));
  fs::write(
    &ledger,
    "{\"kind\":\"read\",\"returned\":120,\"full\":4000}\nwrong\n",
  )
  .expect("invalid savings report");
  let invalid = run(
    Path::new(TOOLU),
    &["ast-grep", "savings", path],
    dir.path(),
    &bin,
  )
  .expect("savings failure");
  assert!(!invalid.status.success());
  assert!(
    String::from_utf8(invalid.stderr)
      .expect("UTF-8 error")
      .contains(":2: invalid ledger line")
  );
}
