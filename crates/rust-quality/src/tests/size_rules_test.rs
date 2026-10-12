use std::path::Path;
use std::process::Command;

use toolu_engine::quality::EditedFile;
use toolu_runtime::env::Env;

use super::{file_too_long, function_too_long, impl_too_long};
use crate::file::{Limits, RsFile, split_lines};

const LIMITS: fn() -> Limits = || Limits {
  file_lines: 4,
  fn_lines: 3,
  impl_lines: 4,
  ..Limits::default()
};

/// `body` written to `<root>/src/a.rs`; `check` sees it under [`LIMITS`].
fn sized<T>(root: &Path, body: &str, check: impl Fn(&RsFile<'_>) -> T) -> T {
  std::fs::create_dir_all(root.join("src")).unwrap();
  std::fs::write(root.join("src/a.rs"), body).unwrap();
  let file = EditedFile {
    path: "src/a.rs".into(),
    absolute: root.join("src/a.rs"),
    removed: false,
  };
  let (limits, env) = (LIMITS(), Env::from_pairs([("PATH", "/usr/bin:/bin")]));
  let lines = split_lines(body);
  check(&RsFile {
    file: &file,
    scope: "/src/a.rs",
    lines,
    limits: &limits,
    env: &env,
    cwd: root,
  })
}

fn repo() -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  assert!(
    Command::new("git")
      .arg("-C")
      .arg(dir.path())
      .args(["init", "-q"])
      .status()
      .unwrap()
      .success()
  );
  dir
}

#[test]
fn a_file_over_its_limit_counts_code_lines_and_names_clippy() {
  let dir = repo();
  let five = "fn a() {}\n// note\n\nfn b() {}\nfn c() {}\nfn d() {}\nfn e() {}\n";
  let found = sized(dir.path(), five, file_too_long).unwrap();
  assert_eq!(
    found,
    "File exceeds 4-line limit: src/a.rs (5 code lines, blanks/comments excluded) — split into submodules"
  );
  std::fs::write(dir.path().join("clippy.toml"), "").unwrap();
  let found = sized(dir.path(), &format!("{five}/* open\n"), file_too_long).unwrap();
  assert!(
    found.contains("(size approximated — an unterminated /*"),
    "{found}"
  );
  assert!(found.ends_with("split into submodules (clippy enforces complexity here)"));
  assert_eq!(sized(dir.path(), "fn a() {}\n", file_too_long), None);
}

#[test]
fn a_function_past_its_limit_is_measured_by_brace_depth() {
  let dir = repo();
  let long = "pub(crate) async fn a() {\n  let s = \"}}}\";\n  let c = '}';\n  x();\n}\n";
  let found = sized(dir.path(), long, |f| function_too_long(f, false));
  assert_eq!(
    found.as_deref(),
    Some("Function too long in src/a.rs (>3 lines) — extract helpers.")
  );
  assert_eq!(
    sized(dir.path(), long, |f| function_too_long(f, true)),
    None,
    "tests are exempt"
  );
  let short = "fn a() {\n  x();\n  y();\n}\n";
  assert_eq!(
    sized(dir.path(), short, |f| function_too_long(f, false)),
    None
  );
  let declared = "fn a();\n\n\n\n\nfn b() {\n}\n";
  assert_eq!(
    sized(dir.path(), declared, |f| function_too_long(f, false)),
    None
  );
  let extern_c = "extern \"C\" fn a() {\n1;\n2;\n3;\n}\n";
  assert!(sized(dir.path(), extern_c, |f| function_too_long(f, false)).is_some());
}

#[test]
fn an_impl_block_past_its_limit_is_reported() {
  let dir = repo();
  let long = "impl<T> X<T> {\n  fn a() {}\n  fn b() {}\n  fn c() {}\n  fn d() {}\n}\n";
  let found = sized(dir.path(), long, |f| impl_too_long(f, false)).unwrap();
  assert_eq!(
    found,
    "Impl block too large in src/a.rs (>4 lines) — split into trait impls or modules."
  );
  assert_eq!(sized(dir.path(), long, |f| impl_too_long(f, true)), None);
  let fitting = "unsafe impl Send for X {\n}\n";
  assert_eq!(
    sized(dir.path(), fitting, |f| impl_too_long(f, false)),
    None
  );
}
