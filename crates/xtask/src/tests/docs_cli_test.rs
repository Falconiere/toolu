use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::{DOCS_DIR, MARKER, run};
use crate::Verdict;
use crate::options::Options;

/// Write an executable through a child `sh`, so no descriptor open for writing
/// lives in this multi-threaded test process (see `launcher_e2e_test.rs`).
fn install(path: &Path, text: &str) {
  let mut child = Command::new("/bin/sh")
    .args(["-c", "cat > \"$1\" && chmod 755 \"$1\"", "sh"])
    .arg(path)
    .stdin(Stdio::piped())
    .spawn()
    .unwrap();
  child
    .stdin
    .take()
    .unwrap()
    .write_all(text.as_bytes())
    .unwrap();
  assert!(child.wait().unwrap().success());
}

/// A `toolu` stand-in with one live command and one planned namespace.
fn stand_in(dir: &Path) -> PathBuf {
  let tree = r#"{"commands":[{"name":"hook","path":["hook"],"about":"Run a hook entry","owner":"toolu","aliases":[],"hidden":false,"placeholder":false,"commands":[]},{"name":"jev","path":["jev"],"about":"Typed judgments","owner":"jev","aliases":[],"hidden":false,"placeholder":false,"commands":[{"name":"planned","path":["jev","planned"],"about":"Not ported yet","hidden":false,"placeholder":true,"commands":[]}]}]}"#;
  let bin = dir.join("toolu");
  install(
    &bin,
    &format!(
      "#!/bin/sh\ncase \"$*\" in\n\"commands --json\") echo '{tree}';;\n\
       \"commands --schema\") echo '{{\"title\":\"schema\"}}';;\n\
       *--help) echo \"Usage: toolu $*\";;\n*) exit 3;;\nesac\n"
    ),
  );
  bin
}

fn options(root: &Path, bin: &Path, check: bool) -> Options {
  Options {
    root: root.to_path_buf(),
    bin: Some(bin.to_path_buf()),
    check,
    ..Options::default()
  }
}

#[test]
fn a_written_tree_checks_clean() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path());
  assert_eq!(run(&options(dir.path(), &bin, false)), Ok(Verdict::Clean));
  let docs = dir.path().join(DOCS_DIR);
  let mut names: Vec<String> = std::fs::read_dir(&docs)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  assert_eq!(
    names,
    [
      "README.md",
      "commands.json",
      "commands.schema.json",
      "hook.md",
      "jev.md"
    ]
  );
  let jev = std::fs::read_to_string(docs.join("jev.md")).unwrap();
  assert!(
    jev.contains("```text\nUsage: toolu jev planned --help\n```"),
    "{jev}"
  );
  assert_eq!(run(&options(dir.path(), &bin, true)), Ok(Verdict::Clean));
}

#[test]
fn a_missing_or_stale_page_is_a_finding() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path());
  assert_eq!(run(&options(dir.path(), &bin, true)), Ok(Verdict::Findings));
  run(&options(dir.path(), &bin, false)).unwrap();
  std::fs::write(dir.path().join(DOCS_DIR).join("jev.md"), "edited by hand\n").unwrap();
  assert_eq!(run(&options(dir.path(), &bin, true)), Ok(Verdict::Findings));
}

#[test]
fn orphaned_generated_pages_are_found_and_removed_but_hand_written_ones_stay() {
  let dir = tempfile::tempdir().unwrap();
  let bin = stand_in(dir.path());
  run(&options(dir.path(), &bin, false)).unwrap();
  let docs = dir.path().join(DOCS_DIR);
  std::fs::write(docs.join("gone.md"), format!("{MARKER}\n\n# old\n")).unwrap();
  std::fs::write(docs.join("installer.md"), "# Installer\n").unwrap();
  assert_eq!(run(&options(dir.path(), &bin, true)), Ok(Verdict::Findings));
  run(&options(dir.path(), &bin, false)).unwrap();
  assert!(!docs.join("gone.md").exists());
  assert_eq!(
    std::fs::read_to_string(docs.join("installer.md")).unwrap(),
    "# Installer\n"
  );
  assert_eq!(run(&options(dir.path(), &bin, true)), Ok(Verdict::Clean));
}

#[test]
fn a_binary_that_fails_is_a_setup_error() {
  let dir = tempfile::tempdir().unwrap();
  let bin = dir.path().join("toolu");
  install(&bin, "#!/bin/sh\necho broken >&2\nexit 2\n");
  let err = run(&options(dir.path(), &bin, true)).unwrap_err();
  assert!(err.contains("commands --json exited"), "{err}");
  assert!(err.ends_with("broken"), "{err}");
  let missing = run(&options(dir.path(), &dir.path().join("absent"), true)).unwrap_err();
  assert!(missing.starts_with("cannot run "), "{missing}");
}
