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
  let bun = on_path("bun").unwrap().to_string_lossy().into_owned();
  let bin = file.to_string_lossy().into_owned();
  let extra = [("TOOLU_BIN", bin.as_str()), ("TOOLU_BUN", bun.as_str())];
  let run = sandbox
    .launch("PreToolUse", "session-start", STARTUP, &extra)
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(
    run
      .stderr
      .starts_with("blocked: toolu plugin: toolu is not installed")
  );
  let context = sandbox
    .launch("SessionStart", "session-start", STARTUP, &extra)
    .unwrap();
  assert_eq!(context.code, 0, "{context:?}");
  assert!(
    system_message(&context)
      .unwrap()
      .starts_with("toolu plugin: toolu is not installed")
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
  probe.arg("--hook-protocol").env("PATH", &path);
  probe.stdout(Stdio::piped()).stderr(Stdio::piped());
  let probe = spawn(&mut probe).unwrap().wait_with_output().unwrap();
  let said = String::from_utf8_lossy(&probe.stderr);
  assert_eq!(said.trim(), "toolu: unknown flag: --hook-protocol");
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

/// Build the real npm wrapper (`@toolu/plugins`' `toolu` bin) from its source.
fn build_wrapper(out: &Path) -> Res<()> {
  let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
  let built = out.with_extension("js");
  let mut bundler = Command::new(on_path("bun")?);
  bundler
    .args([
      "build",
      "tools/toolu-cli/src/cli.ts",
      "--target=node",
      "--outfile",
    ])
    .arg(&built)
    .current_dir(&repo)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let output = spawn(&mut bundler)?.wait_with_output()?;
  if !output.status.success() {
    return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
  }
  script(out, &std::fs::read_to_string(&built)?)
}
