//! Ledger file I/O and location on real files and repositories (`ledger-io.test.ts`).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::jq_text;
use toolu_runtime::process::{Spec, run};

use super::{
  LedgerOptions, Output, ReadLedger, head_branch, ledger_path, read_ledger, write_ledger,
};
use crate::ledger::jq::parse_json;

fn env(home: &Path) -> Env {
  let path = std::env::var("PATH").unwrap();
  let home = home.display().to_string();
  Env::from_pairs([
    ("PATH", path.as_str()),
    ("HOME", home.as_str()),
    ("GIT_CONFIG_NOSYSTEM", "1"),
  ])
}

fn git(dir: &Path, args: &[&str]) {
  let mut spec = Spec::new(["git"]);
  spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
  spec.cwd = Some(dir.to_path_buf());
  spec.env = Some(env(dir));
  let output = run(&spec).unwrap();
  assert_eq!(output.exit_code, 0, "git {args:?}: {}", output.stderr);
}

fn options(cwd: &Path, host: Host) -> LedgerOptions {
  LedgerOptions {
    roots: Roots::new(env(cwd), Some(host)),
    cwd: cwd.to_path_buf(),
    now: SystemTime::now,
  }
}

#[test]
fn a_ledger_is_written_as_jq_prints_it_with_no_stage_left() {
  let dir = tempfile::tempdir().unwrap();
  for (name, value) in [
    (
      "ledger",
      r#"{"version":1,"branch":"feat/x","steps":[{"id":"s1","status":"green","n":1.5}]}"#,
    ),
    (
      "unicode",
      "{\"s\":\"a\u{7f}b é ✓\",\"empty\":[],\"obj\":{}}",
    ),
    ("array", r#"[1,"two",null]"#),
  ] {
    let file = dir.path().join(name).join("nested/l.json");
    let value = parse_json(value).unwrap();
    assert_eq!(write_ledger(&file, &value), Ok(()));
    let written = std::fs::read_to_string(&file).unwrap();
    assert_eq!(written, format!("{}\n", jq_text(&value, true)));
    let names: Vec<_> = std::fs::read_dir(file.parent().unwrap()).unwrap().collect();
    assert_eq!(names.len(), 1, "{name}");
  }
}

#[test]
fn a_file_blocking_the_directory_or_the_rename_is_a_tagged_error() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(dir.path().join("blocker"), "a file").unwrap();
  let file = dir.path().join("blocker/l.json");
  let empty = parse_json("{}").unwrap();
  let blocked = format!(
    "plan-ledger-parse: cannot create ledger dir: {}",
    dir.path().join("blocker").display()
  );
  assert_eq!(write_ledger(&file, &empty), Err(blocked));
  let target = dir.path().join("taken");
  std::fs::create_dir_all(target.join("inside")).unwrap();
  let failed = format!(
    "plan-ledger-parse: atomic mv failed for {}",
    target.display()
  );
  assert_eq!(write_ledger(&target, &empty), Err(failed));
  assert_eq!(
    std::fs::read_dir(dir.path()).unwrap().count(),
    2,
    "no stage is left"
  );
}

#[test]
fn reading_skips_absent_empty_unparseable_and_falsy_documents() {
  let dir = tempfile::tempdir().unwrap();
  let file = dir.path().join("l.json");
  assert_eq!(read_ledger(&file), None);
  for body in ["", "null\n", "false", "{not json"] {
    std::fs::write(&file, body).unwrap();
    assert_eq!(read_ledger(&file), None, "{body:?}");
  }
  for (body, text) in [
    ("0", "0"),
    (
      "{\"version\":1,\"steps\":[]}",
      "{\"version\":1,\"steps\":[]}",
    ),
    ("{\"a\":1}\n\n\n", "{\"a\":1}"),
    ("[]", "[]"),
  ] {
    std::fs::write(&file, body).unwrap();
    let read: Option<ReadLedger> = read_ledger(&file);
    assert_eq!(read.map(|read| read.text).as_deref(), Some(text));
  }
  let as_dir = dir.path().join("d.json");
  std::fs::create_dir_all(as_dir.join("x")).unwrap();
  assert_eq!(read_ledger(&as_dir), None);
}

fn repo() -> (tempfile::TempDir, PathBuf) {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  git(&root, &["init", "-q", "-b", "main"]);
  git(
    &root,
    &[
      "-c",
      "user.name=t",
      "-c",
      "user.email=t@t",
      "commit",
      "-q",
      "--allow-empty",
      "-m",
      "init",
    ],
  );
  std::fs::create_dir_all(root.join("sub/dir")).unwrap();
  (dir, root)
}

#[test]
fn the_ledger_path_follows_the_host_and_branch_slug() {
  let (_dir, root) = repo();
  git(&root, &["checkout", "-q", "-b", "feat/256-x.y"]);
  let cwd = root.join("sub/dir");
  assert_eq!(
    ledger_path(&options(&cwd, Host::Claude)),
    Some(root.join(".claude/tmp/plan-ledger/feat_256-xy.json"))
  );
  git(&root, &["checkout", "-q", "-b", "fix/a"]);
  assert_eq!(
    ledger_path(&options(&cwd, Host::Codex)),
    Some(root.join(".codex/tmp/plan-ledger/fix_a.json"))
  );
  assert_eq!(head_branch(&env(&root), &cwd).as_deref(), Some("fix/a"));
}

#[test]
fn an_unborn_head_or_no_repository_has_no_ledger_path() {
  let dir = tempfile::tempdir().unwrap();
  let root = std::fs::canonicalize(dir.path()).unwrap();
  assert_eq!(ledger_path(&options(&root, Host::Claude)), None);
  git(&root, &["init", "-q", "-b", "main"]);
  assert_eq!(ledger_path(&options(&root, Host::Claude)), None);
}

#[test]
fn output_collects_stdout_and_newline_terminated_stderr_lines() {
  let mut out = Output::default();
  out.stdout("line\n");
  out.stderr("one");
  out.stderr(String::from("two"));
  let result = out.result(1, None);
  assert_eq!(
    (result.exit, result.stdout.as_str(), result.stderr.as_str()),
    (1, "line\n", "one\ntwo\n")
  );
}
