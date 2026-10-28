use std::fs;

use toolu_engine::quality::AstGrepScan;

use crate::contract::Side;
use crate::prepass::tests::{added, empty, env, secret_repo, shaped};
use crate::prepass::{NoticeKind, run_with};
use crate::shape::ChangedLine;

#[test]
fn prepass_astgrep_scans_head_blobs_inside_a_temp_root() {
  let (dir, shaped) = secret_repo();
  let result = run_with(&env(), dir.path(), &shaped, &|files, rules, ctx| {
    let cwd = ctx.cwd.expect("cwd");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].path, "src/lib.rs");
    assert_eq!(files[0].absolute, cwd.join("src/lib.rs"));
    assert!(
      fs::read_to_string(&files[0].absolute)
        .expect("blob")
        .contains("const T")
    );
    assert_ne!(cwd, dir.path());
    assert!(rules.contains("id: review-todo-macro"));
    AstGrepScan::Ok {
      hits: Vec::new(),
      empty: true,
    }
  });
  assert_eq!(result.notices, Vec::new());
}

#[test]
fn prepass_astgrep_a_file_missing_from_head_is_skipped_with_a_notice() {
  let (dir, _) = secret_repo();
  let shaped = shaped("src/gone.rs", vec![added(1, "x")]);
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| AstGrepScan::Missing);
  assert_eq!(result.notices.len(), 1);
  assert_eq!(result.notices[0].kind, NoticeKind::AstGrepFailed);
  assert!(
    result.notices[0]
      .text
      .starts_with("prepass: ast-grep skipped src/gone.rs:")
  );
}

#[test]
fn prepass_astgrep_a_deletion_has_nothing_to_scan() {
  let (dir, _) = secret_repo();
  let removed = ChangedLine {
    side: Side::Old,
    line: 1,
    text: "fn a() {}".to_owned(),
  };
  let shaped = shaped("src/lib.rs", vec![removed]);
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| AstGrepScan::Missing);
  assert_eq!(result, empty());
}

#[test]
fn prepass_astgrep_a_path_starting_with_a_dash_is_skipped_not_passed_as_a_flag() {
  let (dir, _) = secret_repo();
  let shaped = shaped("-rf.rs", vec![added(1, "x")]);
  let result = run_with(&env(), dir.path(), &shaped, &|_, _, _| {
    panic!("no file reaches ast-grep")
  });
  assert_eq!(result.notices.len(), 1);
  assert_eq!(result.notices[0].kind, NoticeKind::AstGrepFailed);
  assert_eq!(
    result.notices[0].text,
    "prepass: ast-grep skipped -rf.rs: path starts with '-' and could be read as a flag"
  );
}
