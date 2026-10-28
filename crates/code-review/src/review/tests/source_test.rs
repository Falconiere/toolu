use std::process::Command;

use toolu_runtime::env::Env;

use super::packages;
use crate::budget::Effort;
use crate::review::mask::Mask;
use crate::shape::shape;

const ISOLATED: [(&str, &str); 3] = [
  ("GIT_CONFIG_NOSYSTEM", "1"),
  ("GIT_CONFIG_GLOBAL", "/dev/null"),
  ("XDG_CONFIG_HOME", "/dev/null"),
];

/// `main` holds `base`; `feature` commits `change` on top. Each entry is a path
/// and its whole text.
fn repo(base: &[(&str, String)], change: &[(&str, String)]) -> tempfile::TempDir {
  let dir = tempfile::tempdir().unwrap();
  let run = |args: &[&str]| {
    let status = Command::new("git")
      .current_dir(dir.path())
      .args(["-c", "user.name=T", "-c", "user.email=t@example.com"])
      .args(args)
      .envs(ISOLATED)
      .status()
      .unwrap();
    assert!(status.success(), "git {args:?}");
  };
  let write = |files: &[(&str, String)]| {
    for (path, text) in files {
      let path = dir.path().join(path);
      std::fs::create_dir_all(path.parent().unwrap()).unwrap();
      std::fs::write(path, text).unwrap();
    }
  };
  run(&["init", "-q", "-b", "main"]);
  write(base);
  run(&["add", "-A"]);
  run(&["commit", "-qm", "base"]);
  run(&["switch", "-qc", "feature"]);
  write(change);
  run(&["add", "-A"]);
  run(&["commit", "-qm", "change"]);
  dir
}

fn env() -> Env {
  ISOLATED
    .iter()
    .fold(Env::process(), |env, (name, value)| env.with(name, value))
}

fn lines(count: usize, word: &str) -> String {
  (0..count).fold(String::new(), |text, index| {
    text + word + " " + &index.to_string() + "\n"
  })
}

#[test]
fn source_is_bounded_per_file_and_per_package_and_never_for_added_files() {
  let small = Effort::Small.budget();
  let dir = repo(
    &[
      ("pkg/a.rs", lines(10, "a")),
      ("pkg/b.rs", lines(10, "b")),
      ("big/c.rs", lines(2000, "c")),
    ],
    &[
      ("pkg/a.rs", lines(11, "a")),
      ("pkg/b.rs", lines(11, "b")),
      ("big/c.rs", lines(2001, "c")),
      ("pkg/new.rs", lines(3, "n")),
    ],
  );
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let (found, notes) = packages(&env, dir.path(), &shaped, small, &Mask::default());
  let by_path = |path: &str| found.iter().find(|package| package.path == path).unwrap();
  assert_eq!(
    by_path("pkg/a.rs").source.as_deref(),
    Some(lines(11, "a").as_str())
  );
  assert!(by_path("pkg/b.rs").source.is_some());
  assert_eq!(
    by_path("pkg/new.rs").source,
    None,
    "the patch is the whole file"
  );
  // Too big to send whole: the window around the appended line instead.
  let window = by_path("big/c.rs").source.clone().unwrap();
  assert!(
    window.starts_with("@@ lines 1971-2001 @@\nc 1970\n"),
    "{window}"
  );
  assert!(window.ends_with("c 1999\nc 2000\n"), "{window}");
  assert!(by_path("big/c.rs").patch.contains("+c 2000"));
  assert_eq!(
    notes,
    [
      "source of big/c.rs: 1 of 1 windows around its changes sent, the whole file is over 8192 bytes"
    ]
  );
  let evidence = by_path("pkg/a.rs").evidence();
  assert!(evidence.starts_with(&by_path("pkg/a.rs").patch) && evidence.ends_with("a 10\n"));
}

/// `bytes` bytes of distinct 64-byte lines, the remainder unterminated.
fn sized(bytes: usize) -> String {
  let text = (0..bytes / 64).fold(String::new(), |text, index| {
    text + "line " + &format!("{index:058}") + "\n"
  });
  text + &"y".repeat(bytes % 64)
}

#[test]
fn a_source_of_exactly_the_cap_is_kept_and_one_byte_more_is_not() {
  let small = Effort::Small.budget();
  let cap = small.source_file_bytes;
  let dir = repo(
    &[
      ("a/fit.rs", "a\n".to_owned()),
      ("b/over.rs", "b\n".to_owned()),
    ],
    &[("a/fit.rs", sized(cap)), ("b/over.rs", sized(cap + 1))],
  );
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let (found, notes) = packages(&env, dir.path(), &shaped, small, &Mask::default());
  let source = |path: &str| {
    let package = found.iter().find(|package| package.path == path).unwrap();
    package.source.as_ref().map(String::len)
  };
  assert_eq!((source("a/fit.rs"), source("b/over.rs")), (Some(cap), None));
  assert_eq!(
    notes,
    [format!("source of b/over.rs left out: over {cap} bytes")]
  );
}

#[test]
fn a_package_over_its_cap_drops_the_next_whole_source() {
  let small = Effort::Small.budget();
  let half = lines(1000, "x");
  assert!(half.len() < small.source_file_bytes && 3 * half.len() > small.source_package_bytes);
  let names = ["pkg/a.rs", "pkg/b.rs", "pkg/c.rs"];
  let base: Vec<(&str, String)> = names.iter().map(|name| (*name, half.clone())).collect();
  let change: Vec<(&str, String)> = names
    .iter()
    .map(|name| (*name, format!("{half}end\n")))
    .collect();
  let dir = repo(&base, &change);
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let (found, notes) = packages(&env, dir.path(), &shaped, small, &Mask::default());
  let kept = found
    .iter()
    .filter(|package| package.source.is_some())
    .count();
  assert_eq!(kept, 2);
  assert_eq!(notes.len(), 1);
  assert!(
    notes[0].contains("package \"pkg\" over 16384 bytes"),
    "{notes:?}"
  );
}

#[test]
fn files_left_out_of_every_chunk_spend_no_package_budget() {
  let small = Effort::Small.budget();
  let half = lines(1000, "x");
  let names = ["pkg/a.rs", "pkg/b.rs", "pkg/c.rs"];
  let base: Vec<(&str, String)> = names.iter().map(|name| (*name, half.clone())).collect();
  let change: Vec<(&str, String)> = names
    .iter()
    .map(|name| (*name, format!("{half}end\n")))
    .collect();
  let dir = repo(&base, &change);
  let env = env();
  let mut shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let last = shaped.files.len() - 1;
  // What a `--path pkg/c.rs` scope leaves: only the last file is chunked.
  for chunk in &mut shaped.chunks {
    chunk.files.retain(|index| *index == last);
  }
  let (found, notes) = packages(&env, dir.path(), &shaped, small, &Mask::default());
  assert!(found[last].source.is_some(), "{notes:?}");
  assert!(found[..last].iter().all(|package| package.source.is_none()));
  assert!(notes.is_empty(), "{notes:?}");
}

#[test]
fn a_secret_on_an_unchanged_line_is_masked_in_the_source() {
  let token = format!("ghp_{}", "Ab1".repeat(12));
  let base = format!("const T: &str = \"{token}\";\nfn a() {{}}\n");
  let change = format!("const T: &str = \"{token}\";\nfn a() {{ b() }}\n");
  let dir = repo(&[("src/a.rs", base)], &[("src/a.rs", change)]);
  let env = env();
  let shaped = shape(&env, dir.path(), Some("main")).unwrap();
  let budget = Effort::Medium.budget();
  let (found, _) = packages(&env, dir.path(), &shaped, budget, &Mask::default());
  let source = found[0].source.as_deref().unwrap();
  assert!(!source.contains(&token), "{source}");
  assert!(source.ends_with("fn a() { b() }\n"), "{source}");
}
