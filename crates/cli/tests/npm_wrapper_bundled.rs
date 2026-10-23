//! The npm wrapper's bundled binary (`bin/<os>-<arch>/toolu`): it runs with no
//! system binary (AC-1), loses to an explicit `TOOLU_BIN` (AC-2), and a corrupt
//! bundle falls through to the system search (AC-3). Real `node`, the real
//! built `toolu` where a binary runs, shell stand-ins elsewhere, on the shared
//! `helpers/npm.rs` harness.

#[path = "helpers/npm.rs"]
mod npm;

use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

use npm::{Res, SYSTEM, TOOLU, native_stub, run, script, temp, wrapper};

/// This host's bundled-binary directory (`bin/<os>-<arch>`), as `cli.js` maps
/// it; `None` where the package ships no binary.
fn bundled_dir() -> Option<&'static str> {
  match (std::env::consts::OS, std::env::consts::ARCH) {
    ("macos", "aarch64") => Some("darwin-arm64"),
    ("macos", "x86_64") => Some("darwin-amd64"),
    ("linux", "aarch64") => Some("linux-arm64"),
    ("linux", "x86_64") => Some("linux-amd64"),
    _ => None,
  }
}

/// A copy of the wrapper package with an empty `bin/<host>` slot; the caller
/// stages the bundled binary there. Returns the copied `cli.js` and the slot.
fn package_skeleton(root: &Path) -> Res<(PathBuf, PathBuf)> {
  let dir = root.join("pkg");
  std::fs::create_dir_all(&dir)?;
  std::fs::copy(wrapper(), dir.join("cli.js"))?;
  std::fs::write(
    dir.join("package.json"),
    "{\"type\":\"module\",\"version\":\"9.9.9\"}",
  )?;
  let staged = dir
    .join("bin")
    .join(bundled_dir().ok_or("no bundled dir")?)
    .join("toolu");
  std::fs::create_dir_all(staged.parent().ok_or("no parent")?)?;
  Ok((dir.join("cli.js"), staged))
}

/// A planned dry run and the exact output the real binary prints for it.
const DRY_RUN: &[&str] = &["install", "--dry-run", "--host", "claude", "jev"];
const PLANNED: &str = "Planned commands (nothing was run):\n  claude plugin install jev@toolu\n";

#[test]
fn the_staged_bundled_binary_runs_without_a_system_binary() {
  let (_dir, root) = temp().unwrap();
  let (cli, staged) = package_skeleton(&root).unwrap();
  symlink(Path::new(TOOLU), &staged).unwrap();
  let home = root.display().to_string();
  let ran = run(&cli, DRY_RUN, &[("PATH", SYSTEM), ("HOME", &home)], &root).unwrap();
  assert_eq!(
    (ran.code, ran.stderr.as_str(), ran.stdout.as_str()),
    (Some(0), "", PLANNED),
    "{ran:?}"
  );
}

#[test]
fn the_bundled_slot_wins_over_the_system_search() {
  let (_dir, root) = temp().unwrap();
  let log = root.join("argv.log");
  let (cli, staged) = package_skeleton(&root).unwrap();
  native_stub(&staged, &log, "exit 7").unwrap();
  let system_log = root.join("system.log");
  native_stub(&root.join("system/toolu"), &system_log, "exit 5").unwrap();
  let path = format!("{}:{SYSTEM}", root.join("system").display());
  let home = root.display().to_string();
  let ran = run(&cli, &["list"], &[("PATH", &path), ("HOME", &home)], &root).unwrap();
  assert_eq!((ran.code, ran.stderr.as_str()), (Some(7), ""), "{ran:?}");
  assert_eq!(std::fs::read_to_string(&log).unwrap(), "plugins list\n");
  assert!(!system_log.exists(), "the system binary ran");
}

#[test]
fn toolu_bin_wins_over_a_staged_bundled_binary() {
  let (_dir, root) = temp().unwrap();
  let log = root.join("argv.log");
  let (cli, staged) = package_skeleton(&root).unwrap();
  symlink(Path::new(TOOLU), &staged).unwrap();
  let over = native_stub(&root.join("override"), &log, "exit 3").unwrap();
  let over = over.display().to_string();
  let home = root.display().to_string();
  let env = [("PATH", SYSTEM), ("HOME", &home), ("TOOLU_BIN", &over)];
  let ran = run(&cli, &["install", "jev"], &env, &root).unwrap();
  assert_eq!((ran.code, ran.stderr.as_str()), (Some(3), ""), "{ran:?}");
  assert_eq!(
    std::fs::read_to_string(&log).unwrap(),
    "plugins install jev\n"
  );
}

#[test]
fn a_corrupt_bundled_binary_falls_through_to_the_system_search() {
  let (_dir, root) = temp().unwrap();
  let (cli, staged) = package_skeleton(&root).unwrap();
  script(&staged, "exit 2").unwrap();
  let system = root.join("system");
  std::fs::create_dir_all(&system).unwrap();
  symlink(Path::new(TOOLU), system.join("toolu")).unwrap();
  let path = format!("{}:{SYSTEM}", system.display());
  let home = root.display().to_string();
  let env = [("PATH", path.as_str()), ("HOME", &home)];
  let ran = run(&cli, DRY_RUN, &env, &root).unwrap();
  assert_eq!(
    (ran.code, ran.stderr.as_str(), ran.stdout.as_str()),
    (Some(0), "", PLANNED),
    "{ran:?}"
  );
}
