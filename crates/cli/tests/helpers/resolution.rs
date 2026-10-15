//! Which candidate the launcher picks: `TOOLU_BIN` alone, and never a program
//! that fails the native signature (AC-6, AC-7).

use std::path::Path;
use std::process::{Command, Stdio};

use crate::sandbox::{
  REDUCED_PATH, Res, STARTUP, Sandbox, TOOLU, copy_executable, on_path, script, spawn,
  system_message,
};

#[test]
fn a_non_executable_toolu_bin_fails_closed_even_with_a_valid_install() {
  let sandbox = Sandbox::new().unwrap();
  sandbox.install().unwrap();
  let file = sandbox.home.join("toolu-copy");
  std::fs::write(&file, "not a program").unwrap();
  let bin = file.to_string_lossy().into_owned();
  let extra = [("TOOLU_BIN", bin.as_str())];
  let run = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &extra)
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(
    run
      .stderr
      .starts_with("blocked: toolu plugin: no usable native toolu")
  );
  assert!(
    run
      .stderr
      .contains("If TOOLU_BIN is set, it must point to a native toolu")
  );
  let context = sandbox
    .launch("SessionStart", "session-start", STARTUP, &extra)
    .unwrap();
  assert_eq!(context.code, 0, "{context:?}");
  assert!(
    system_message(&context)
      .unwrap()
      .starts_with("toolu plugin: no usable native toolu")
  );
  assert!(
    system_message(&context)
      .unwrap()
      .contains("If TOOLU_BIN is set, it must point to a native toolu")
  );
}

#[test]
fn a_valid_toolu_bin_wins_and_an_empty_one_counts_as_unset() {
  let sandbox = Sandbox::new().unwrap();
  let installed = sandbox.install().unwrap();
  let chosen = sandbox.home.join("chosen");
  std::fs::create_dir_all(&chosen).unwrap();
  copy_executable(Path::new(TOOLU), &chosen.join("toolu")).unwrap();
  let chosen = std::fs::canonicalize(chosen.join("toolu")).unwrap();
  let bin = chosen.to_string_lossy().into_owned();
  let explicit = sandbox
    .launch(
      "SessionStart",
      "session-start",
      STARTUP,
      &[("TOOLU_BIN", &bin)],
    )
    .unwrap();
  assert!(
    system_message(&explicit)
      .unwrap()
      .ends_with(&format!("at {bin}"))
  );
  let empty = sandbox
    .launch(
      "SessionStart",
      "session-start",
      STARTUP,
      &[("TOOLU_BIN", "")],
    )
    .unwrap();
  assert!(
    system_message(&empty)
      .unwrap()
      .ends_with(&format!("at {}", installed.display()))
  );
}

#[test]
fn the_npm_wrapper_first_on_path_is_skipped() {
  let sandbox = Sandbox::new().unwrap();
  let installed = sandbox.install().unwrap();
  let wrapper_dir = sandbox.home.join("npm bin");
  std::fs::create_dir_all(&wrapper_dir).unwrap();
  build_wrapper(&wrapper_dir.join("toolu")).unwrap();
  let node = on_path("node").unwrap();
  let node_dir = node.parent().ok_or("node has no directory").unwrap();
  let path = format!(
    "{}:{}:{REDUCED_PATH}",
    wrapper_dir.display(),
    node_dir.display()
  );
  let mut probe = Command::new(wrapper_dir.join("toolu"));
  // TOOLU_BIN names the wrapper itself, so it is the only candidate: no fixed
  // directory, `~/.local/bin` or PATH entry can decide the case.
  let wrapper = wrapper_dir.join("toolu");
  probe
    .arg("--hook-protocol")
    .env_clear()
    .env("PATH", &path)
    .env("TOOLU_BIN", &wrapper);
  probe.stdout(Stdio::piped()).stderr(Stdio::piped());
  let probe = spawn(&mut probe).unwrap().wait_with_output().unwrap();
  // The wrapper never answers the native probe: it is not the native CLI, and
  // it skips itself when it looks for one (#438): it names the install
  // commands and exits 69.
  let said = String::from_utf8_lossy(&probe.stderr);
  assert_eq!(probe.status.code(), Some(69), "{said}");
  assert!(
    said.starts_with("toolu-plugins: the native toolu binary was not found"),
    "{said}"
  );
  assert!(
    said.contains("curl -fsSL https://get.toolu.sh/pkg/toolu/install | bash"),
    "{said}"
  );
  assert_eq!(probe.stdout, b"");
  let run = sandbox
    .launch("SessionStart", "session-start", STARTUP, &[("PATH", &path)])
    .unwrap();
  assert!(
    system_message(&run)
      .unwrap()
      .ends_with(&format!("at {}", installed.display())),
    "{run:?}"
  );
}

/// Copy the real npm wrapper (`@toolu/plugins`' `toolu-plugins` bin) to `out`,
/// as a user's leftover `toolu` copy of it would run.
fn build_wrapper(out: &Path) -> Res<()> {
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  script(
    out,
    &std::fs::read_to_string(repo.join("tools/toolu-cli/npm/cli.js"))?,
  )
}
