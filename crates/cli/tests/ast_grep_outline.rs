//! `toolu ast-grep outline` against a real Git repository and the real ast-grep.

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

fn repo() -> Res<tempfile::TempDir> {
  let dir = tempfile::tempdir()?;
  let root = dir.path();
  fs::create_dir(root.join("src"))?;
  fs::write(
    root.join("src/a.rs"),
    "/// Adds one. Then more.\npub fn add(x: i32) -> i32 {\n  fn local() {}\n  x + 1\n}\n\npub struct Unit;\n",
  )?;
  fs::write(
    root.join("src/b.ts"),
    "export class Box {\n  open(): void {}\n}\n",
  )?;
  fs::write(
    root.join("src/c.py"),
    "def _private():\n    pass\n\ndef shown():\n    \"\"\"Shown here.\"\"\"\n",
  )?;
  fs::write(root.join("src/ignored.rs"), "pub fn hidden() {}\n")?;
  fs::write(root.join("src/README.md"), "# notes\n")?;
  fs::write(root.join(".gitignore"), "src/ignored.rs\n")?;
  git(root, &["init", "-q"])?;
  git(root, &["add", "src/a.rs", "src/b.ts", ".gitignore"])?;
  Ok(dir)
}

fn git(root: &Path, args: &[&str]) -> Res<()> {
  let status = Command::new("git").args(args).current_dir(root).status()?;
  if status.success() {
    Ok(())
  } else {
    Err(format!("git {args:?} failed").into())
  }
}

fn outline(root: &Path, args: &[&str], path: Option<&Path>) -> Res<Output> {
  let mut command = Command::new(TOOLU);
  command
    .arg("ast-grep")
    .arg("outline")
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
fn ast_grep_outline_prints_a_file_and_a_directory_map() {
  let dir = repo().unwrap();
  let out = outline(dir.path(), &["src/a.rs", "src"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  assert_eq!(
    text(&out.stdout),
    "src/a.rs: 7 lines, 2 items
1-5 pub fn add(x: i32) -> i32 // Adds one.
7-7 pub struct Unit

src: 3 files
src/a.rs: 7 lines, 2 items: add, Unit
src/b.ts: 3 lines, 2 items: Box
src/c.py: 5 lines, 2 items: shown
"
  );
}

#[test]
fn ast_grep_outline_json_is_one_document() {
  let dir = repo().unwrap();
  let out = outline(dir.path(), &["--json", "src/b.ts", "src"], None).unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let doc: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
  let file = &doc["paths"][0];
  assert_eq!(file["path"], "src/b.ts");
  assert_eq!(file["items"][1]["signature"], "open(): void");
  assert_eq!(file["items"][1]["depth"], 1);
  assert_eq!(file["omitted"], 0);
  let map = &doc["paths"][1];
  assert_eq!(map["dir"], "src");
  assert_eq!(map["files"][2]["names"], serde_json::json!(["shown"]));
}

#[test]
fn ast_grep_outline_without_ast_grep_exits_69() {
  let dir = repo().unwrap();
  let bin = dir.path().join("bin");
  fs::create_dir(&bin).unwrap();
  symlink(on_path("git").unwrap(), bin.join("git")).unwrap();
  let out = outline(dir.path(), &["src"], Some(&bin)).unwrap();
  assert_eq!(out.status.code(), Some(69));
  assert!(
    text(&out.stderr).contains("brew install ast-grep"),
    "{}",
    text(&out.stderr)
  );
  assert_eq!(text(&out.stdout), "");
}

#[test]
fn ast_grep_outline_refuses_unsupported_and_missing_paths() {
  let dir = repo().unwrap();
  let unsupported = outline(dir.path(), &["src/README.md"], None).unwrap();
  assert_eq!(unsupported.status.code(), Some(64));
  assert_eq!(
    text(&unsupported.stderr),
    "ast-grep outline: unsupported language: src/README.md\n"
  );
  let missing = outline(dir.path(), &["src/none.rs"], None).unwrap();
  assert_eq!(missing.status.code(), Some(64));
  assert_eq!(
    text(&missing.stderr),
    "ast-grep outline: src/none.rs: no such file or directory\n"
  );
}
