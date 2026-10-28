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
  assert_eq!(by_path("big/c.rs").source, None);
  assert!(by_path("big/c.rs").patch.contains("+c 2000"));
  assert_eq!(notes, ["source of big/c.rs left out: over 8192 bytes"]);
  let evidence = by_path("pkg/a.rs").evidence();
  assert!(evidence.starts_with(&by_path("pkg/a.rs").patch) && evidence.ends_with("a 10\n"));
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
