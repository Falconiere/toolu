//! `toolu ast-grep symbol` against a real Git repository and the real ast-grep.

use std::error::Error;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Res<T> = Result<T, Box<dyn Error>>;
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

fn on_path(name: &str) -> Res<PathBuf> {
  let path = std::env::var_os("PATH").ok_or("PATH missing")?;
  std::env::split_paths(&path)
    .map(|dir| dir.join(name))
    .find(|path| path.is_file())
    .ok_or_else(|| format!("real {name} is required").into())
}

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git").args(args).current_dir(root).status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("git {args:?} failed").into())
  }
}

/// `add` defined once in `src/a.rs`, called in three other files and an untracked one.
fn repo() -> Res<tempfile::TempDir> {
  let dir = tempfile::tempdir()?;
  let root = dir.path();
  fs::create_dir(root.join("src"))?;
  fs::write(
    root.join("src/a.rs"),
    "/// Adds one.\npub fn add(x: i32) -> i32 {\n  x + 1\n}\n",
  )?;
  fs::write(
    root.join("src/b.rs"),
    "use crate::a::add;\n\nfn main() {\n  let y = add(1);\n}\n",
  )?;
  fs::write(
    root.join("src/c.ts"),
    "export function twice(n: number): number {\n  return add(add(n));\n}\n",
  )?;
  fs::write(root.join("src/d.py"), "add(2)\n")?;
  fs::write(root.join("notes.md"), "call add here\n")?;
  git(root, &["init", "-q"])?;
  git(
    root,
    &["add", "src/a.rs", "src/b.rs", "src/c.ts", "notes.md"],
  )?;
  Ok(dir)
}

fn symbol(root: &Path, args: &[&str], path: Option<&Path>) -> Res<Output> {
  let mut command = Command::new(TOOLU);
  command
    .arg("ast-grep")
    .arg("symbol")
    .args(args)
    .current_dir(root);
  if let Some(path) = path {
    command.env("PATH", path);
  }
  Ok(command.output()?)
}

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn ast_grep_symbol_prints_definition_and_enclosed_references() {
  let dir = repo().unwrap();
  let out = symbol(dir.path(), &["add"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  assert_eq!(
    text(&out.stdout),
    "add: 1 definition, 4 references in 4 files
def src/a.rs:1-4 pub fn add(x: i32) -> i32
src/b.rs
  1: use crate::a::add;
  4 in main 3-5: let y = add(1);
src/c.ts
  2 in twice 1-3: return add(add(n));
src/d.py
  1: add(2)
"
  );
}

#[test]
fn ast_grep_symbol_given_paths_replace_the_language_filter() {
  let dir = repo().unwrap();
  let out = symbol(dir.path(), &["add", "notes.md", "src/d.py"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  assert_eq!(
    text(&out.stdout),
    "add: 0 definitions, 2 references in 2 files\nnotes.md\n  1: call add here\nsrc/d.py\n  1: add(2)\n"
  );
}

#[test]
fn ast_grep_symbol_caps_sixty_call_sites() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path();
  fs::write(root.join("a.rs"), "pub fn go() {}\n").unwrap();
  for file in 0..6 {
    let body: String = (0..10).map(|_| "  go();\n").collect();
    fs::write(
      root.join(format!("f{file}.rs")),
      format!("fn run() {{\n{body}}}\n"),
    )
    .unwrap();
  }
  git(root, &["init", "-q"]).unwrap();
  let out = symbol(root, &["go"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let stdout = text(&out.stdout);
  let lines: Vec<&str> = stdout.lines().collect();
  assert_eq!(
    lines.first(),
    Some(&"go: 1 definition, 60 references in 7 files")
  );
  assert_eq!(lines.get(1), Some(&"def a.rs:1-1 pub fn go()"));
  assert_eq!(
    lines
      .iter()
      .filter(|line| line.contains(" in run 1-12: go();"))
      .count(),
    40
  );
  assert_eq!(
    lines
      .iter()
      .filter(|line| **line == "  ... 2 more in this file")
      .count(),
    5
  );
  assert!(!stdout.contains("f5.rs"), "{stdout}");
  assert_eq!(
    lines.last(),
    Some(&"Narrow: toolu ast-grep symbol go <path>")
  );
}

#[test]
fn ast_grep_symbol_unknown_name_exits_1() {
  let dir = repo().unwrap();
  let out = symbol(dir.path(), &["nope"], None).unwrap();
  assert_eq!(out.status.code(), Some(1));
  assert_eq!(text(&out.stdout), "");
  assert_eq!(text(&out.stderr), "ast-grep symbol: no match for nope\n");
  let empty = symbol(dir.path(), &[""], None).unwrap();
  assert_eq!(empty.status.code(), Some(64));
}

#[test]
fn ast_grep_symbol_without_ast_grep_still_prints_references() {
  let dir = repo().unwrap();
  let bin = dir.path().join("bin");
  fs::create_dir(&bin).unwrap();
  symlink(on_path("git").unwrap(), bin.join("git")).unwrap();
  let out = symbol(dir.path(), &["add"], Some(&bin)).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let stdout = text(&out.stdout);
  assert!(
    stdout.starts_with(
      "add: 0 definitions, 5 references in 4 files\nsrc/a.rs\n  2: pub fn add(x: i32) -> i32 {\n"
    ),
    "{stdout}"
  );
  assert!(
    !stdout.contains(" in main") && !stdout.contains(" in twice"),
    "{stdout}"
  );
  assert!(stdout.ends_with("\nnote: ast-grep not found on PATH; definitions and enclosing items are skipped (install: brew install ast-grep)\n"), "{stdout}");
}

#[test]
fn ast_grep_symbol_json_is_one_document() {
  let dir = repo().unwrap();
  let out = symbol(dir.path(), &["--json", "add"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  assert_eq!(
    doc["definitions"][0]["signature"],
    "pub fn add(x: i32) -> i32"
  );
  assert_eq!(doc["definitions"][0]["line"], 2);
  assert_eq!(
    doc["references"][1]["in"],
    serde_json::json!({"name": "main", "start": 3, "end": 5})
  );
  assert_eq!(doc["references"][0]["in"], serde_json::Value::Null);
  assert_eq!(
    doc["totals"],
    serde_json::json!({"definitions": 1, "references": 4, "files": 4})
  );
  assert_eq!(doc["truncated"], false);
  assert_eq!(doc["note"], serde_json::Value::Null);
}
