//! No Bun fallback (#440): a Bun on `PATH`, in `~/.bun/bin` or in `TOOLU_BUN`,
//! next to a shipped-looking bundle, is never run.

use std::path::PathBuf;

use crate::sandbox::{REDUCED_PATH, Res, STARTUP, Sandbox, script, system_message};

/// A `bun` that records being run, reachable every way the old launcher looked
/// for one, and a bundle where the old launcher expected it. Returns the
/// marker file and the extra environment.
fn bait(sandbox: &Sandbox) -> Res<(PathBuf, Vec<(String, String)>)> {
  let marker = sandbox.home.join("bun-ran");
  let bin = sandbox.home.join(".bun/bin");
  std::fs::create_dir_all(&bin)?;
  let bun = bin.join("bun");
  script(
    &bun,
    &format!("#!/bin/sh\ntouch '{}'\nexit 0\n", marker.display()),
  )?;
  let dist = sandbox.root.join("hooks/dist");
  std::fs::create_dir_all(&dist)?;
  for name in ["pre-tools", "session-start"] {
    std::fs::write(dist.join(format!("{name}.js")), "process.exit(0);")?;
  }
  let env = vec![
    ("TOOLU_BUN".to_owned(), bun.to_string_lossy().into_owned()),
    ("PATH".to_owned(), format!("{}:{REDUCED_PATH}", bin.display())),
  ];
  Ok((marker, env))
}

fn pairs(env: &[(String, String)]) -> Vec<(&str, &str)> {
  env
    .iter()
    .map(|(key, value)| (key.as_str(), value.as_str()))
    .collect()
}

#[test]
fn missing_binary_blocks_even_with_bun() {
  let sandbox = Sandbox::new().unwrap();
  let (marker, env) = bait(&sandbox).unwrap();
  let run = sandbox
    .launch("PreToolUse", "pre-tools", "{}", &pairs(&env))
    .unwrap();
  assert_eq!(run.code, 2, "{run:?}");
  assert!(
    run
      .stderr
      .starts_with("blocked: toolu plugin: no usable native toolu"),
    "{run:?}"
  );
  assert!(!marker.exists(), "the launcher ran bun");
}

#[test]
fn missing_binary_advises_at_session_start_even_with_bun() {
  let sandbox = Sandbox::new().unwrap();
  let (marker, env) = bait(&sandbox).unwrap();
  let run = sandbox
    .launch("SessionStart", "session-start", STARTUP, &pairs(&env))
    .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert!(
    system_message(&run)
      .unwrap()
      .starts_with("toolu plugin: no usable native toolu")
  );
  assert!(!marker.exists(), "the launcher ran bun");
}

#[test]
fn toolu_runtime_bun_is_ignored_and_native_runs() {
  let sandbox = Sandbox::new().unwrap();
  let installed = sandbox.install().unwrap();
  let (marker, mut env) = bait(&sandbox).unwrap();
  env.push(("TOOLU_RUNTIME".to_owned(), "bun".to_owned()));
  let run = sandbox
    .launch("SessionStart", "session-start", STARTUP, &pairs(&env))
    .unwrap();
  assert_eq!(run.code, 0, "{run:?}");
  assert!(
    system_message(&run)
      .unwrap()
      .ends_with(&format!("at {}", installed.display())),
    "{run:?}"
  );
  assert!(!marker.exists(), "the launcher ran bun");
}
