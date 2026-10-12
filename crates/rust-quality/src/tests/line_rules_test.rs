use std::path::Path;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

use super::{has_unsafe, suppression, unsafe_code};
use crate::file::{Limits, RsFile};

fn lines_of(path: &str, lines: &[&str], exempt: &[&str]) -> (Option<String>, Option<String>) {
  let limits = Limits {
    unsafe_exemptions: exempt.iter().map(|name| (*name).to_owned()).collect(),
    ..Limits::default()
  };
  let env = Env::default();
  let file = EditedFile {
    path: path.into(),
    absolute: Path::new("/r").join(path),
    removed: false,
  };
  let f = RsFile {
    file: &file,
    scope: path,
    lines: lines.to_vec(),
    limits: &limits,
    env: &env,
    cwd: Path::new("/r"),
  };
  (suppression(&f), unsafe_code(&f))
}

#[test]
fn lint_suppression_is_forbidden_and_tests_keep_their_inner_header() {
  let message = "Forbidden lint suppression (#[allow]/#[expect]/cfg_attr allow) in /r/src/a.rs — remove it and fix the underlying warning in code. For unsafe_code, override in Cargo.toml [lints.rust].";
  for line in [
    "#[allow(dead_code)]",
    "  #![expect(x)]",
    "#[cfg_attr(test, allow(x))]",
  ] {
    assert_eq!(
      lines_of("/r/src/a.rs", &[line], &[]).0.as_deref(),
      Some(message),
      "{line}"
    );
  }
  assert_eq!(
    lines_of("/r/src/a.rs", &["#[cfg_attr(test, allowed)]"], &[]).0,
    None
  );
  assert_eq!(lines_of("/r/tests/a.rs", &["#![allow(x)]"], &[]).0, None);
  let outer = lines_of("/r/tests/a.rs", &["#[allow(x)]"], &[]).0.unwrap();
  assert!(outer.ends_with(
    "[lints.rust]. In a test file only the file-level #![allow(...)] header is accepted."
  ));
}

/// `lines` with `UNSAFE` spelled as the keyword, so this file holds none.
fn spelled(lines: &[&str]) -> Vec<String> {
  lines
    .iter()
    .map(|line| line.replace("UNSAFE", "unsafe"))
    .collect()
}

fn unsafe_in(lines: &[&str]) -> bool {
  let owned = spelled(lines);
  has_unsafe(&owned.iter().map(String::as_str).collect::<Vec<_>>())
}

#[test]
fn unsafe_outside_comments_is_found() {
  assert!(unsafe_in(&["  UNSAFE { x() }"]));
  assert!(unsafe_in(&["pub UNSAFE fn f() {}"]));
  assert!(!unsafe_in(&[
    "// UNSAFE { x() }",
    "let s = 1; // UNSAFE fn"
  ]));
  assert!(!unsafe_in(&["/* UNSAFE {", "  UNSAFE fn", "*/ fine();"]));
  assert!(unsafe_in(&["/* a */ UNSAFE { /* b */"]));
  assert!(unsafe_in(&["/* open", "*/ UNSAFE {"]));
  assert!(!unsafe_in(&["not_UNSAFE { }", "UNSAFEly fn"]));
}

#[test]
fn an_exempt_crate_may_use_unsafe() {
  let owned = spelled(&["UNSAFE { x() }"]);
  let body = [owned[0].as_str()];
  let found = lines_of("crates/ffi/src/a.rs", &body, &[]).1.unwrap();
  assert_eq!(
    found,
    "Forbidden unsafe code in crates/ffi/src/a.rs — refactor to safe alternative. Add crate to settings/rust-unsafe-exemptions.txt if it legitimately needs unsafe (FFI, sandboxing)."
  );
  assert_eq!(lines_of("crates/ffi/src/a.rs", &body, &["ffi"]).1, None);
  assert!(
    lines_of("crates/ffi/src/a.rs", &body, &["", "other"])
      .1
      .is_some()
  );
}
