use std::path::{Path, PathBuf};

use super::run;
use crate::Verdict;
use crate::options::Options;
use crate::tests::install;

/// A `toolu` stand-in that reports `at` as its path and blocks `PreToolUse`.
fn stand_in(home: &Path, at: &str) -> PathBuf {
  let bin = home.join(".local/bin/toolu");
  let text = format!(
    "#!/bin/sh\ncase \"$1\" in\n--version) echo 'toolu 9.9.9';;\n--hook-protocol) echo 1;;\n\
     *) cat >/dev/null; if [ \"$4\" = PreToolUse ]; then echo 'blocked: toolu plugin: hook protocol 2 needs a newer toolu - toolu 9.9.9 speaks protocol 1; upgrade it: x' >&2; exit 2; fi\n\
     printf '%s\\n' '{{\"systemMessage\":\"toolu runtime: native 9.9.9 at {at}\"}}';;\nesac\n"
  );
  install(&bin, &text);
  bin
}

fn options(home: &Path, bin: &Path) -> Options {
  Options {
    root: PathBuf::from("."),
    bin: Some(bin.to_path_buf()),
    home: Some(home.to_path_buf()),
    ..Options::default()
  }
}

#[test]
fn a_binary_in_local_bin_that_reports_itself_passes() {
  let home = tempfile::tempdir().unwrap();
  let home = std::fs::canonicalize(home.path()).unwrap();
  let at = home.join(".local/bin/toolu");
  let bin = stand_in(&home, &at.display().to_string());
  assert_eq!(run(&options(&home, &bin)), Ok(Verdict::Clean));
}

#[test]
fn a_binary_that_reports_another_path_is_a_finding() {
  let home = tempfile::tempdir().unwrap();
  let bin = stand_in(home.path(), "/somewhere/else/toolu");
  assert_eq!(run(&options(home.path(), &bin)), Ok(Verdict::Findings));
}

#[test]
fn a_binary_outside_the_fixed_directories_is_refused() {
  let home = tempfile::tempdir().unwrap();
  let bin = home.path().join("bin/toolu");
  install(&bin, "#!/bin/sh\n");
  let err = run(&options(home.path(), &bin)).unwrap_err();
  assert!(
    err.ends_with("is not toolu in a fixed install directory"),
    "{err}"
  );
  let missing = home.path().join(".local/bin/toolu");
  assert!(
    run(&options(home.path(), &missing))
      .unwrap_err()
      .ends_with("does not exist")
  );
  let none = Options {
    bin: None,
    ..options(home.path(), &missing)
  };
  assert!(
    run(&none)
      .unwrap_err()
      .starts_with("launcher-e2e needs --bin")
  );
}
