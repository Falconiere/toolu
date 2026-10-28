//! The `limactl` command lines the Lima backend runs, built without running
//! anything, so each one is checked as data. `program` is the `limactl` found on
//! the context's `PATH`; automation passes `--tty=false`.

use std::path::{Path, PathBuf};

use toolu_runtime::env::Env;

use crate::provision::{GUEST_CACHE, VmSpec};

/// The command Lima installs.
pub(crate) const PROGRAM: &str = "limactl";
/// How long Lima may take to boot a VM before giving up.
const BOOT_TIMEOUT: &str = "30m";

/// The first executable `limactl` on `env`'s `PATH`.
pub(crate) fn find(env: &Env) -> Option<PathBuf> {
  use std::os::unix::fs::PermissionsExt as _;
  env
    .get("PATH")?
    .split(':')
    .filter(|dir| !dir.is_empty())
    .map(|dir| Path::new(dir).join(PROGRAM))
    .find(|path| {
      std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

fn argv(program: &Path, words: &[&str]) -> Vec<String> {
  let mut all = vec![program.to_string_lossy().into_owned()];
  all.extend(words.iter().map(|word| (*word).to_owned()));
  all
}

/// `limactl --version`.
pub(crate) fn version(program: &Path) -> Vec<String> {
  argv(program, &["--version"])
}

/// Every instance, one JSON document per line.
pub(crate) fn list(program: &Path) -> Vec<String> {
  argv(program, &["list", "--format", "json"])
}

/// Create VM `name` from the Lima file `yaml` and boot it.
pub(crate) fn create(program: &Path, name: &str, yaml: &Path) -> Vec<String> {
  let mut words = argv(
    program,
    &[
      "--tty=false",
      "start",
      "--name",
      name,
      "--timeout",
      BOOT_TIMEOUT,
    ],
  );
  words.push(yaml.to_string_lossy().into_owned());
  words
}

/// Boot the stopped VM `name`.
pub(crate) fn start(program: &Path, name: &str) -> Vec<String> {
  argv(
    program,
    &["--tty=false", "start", "--timeout", BOOT_TIMEOUT, name],
  )
}

/// Stop VM `name`.
pub(crate) fn stop(program: &Path, name: &str) -> Vec<String> {
  argv(program, &["--tty=false", "stop", name])
}

/// Delete VM `name` with its disk.
pub(crate) fn delete(program: &Path, name: &str) -> Vec<String> {
  argv(program, &["--tty=false", "delete", "--force", name])
}

/// Give the stopped VM `name` the CPUs, memory and mounts of `spec`; the disk
/// is only set when the VM is created.
pub(crate) fn edit(program: &Path, name: &str, spec: &VmSpec) -> Vec<String> {
  let mut words = argv(program, &["--tty=false", "edit", name]);
  words.extend([
    "--cpus".to_owned(),
    spec.cpus.to_string(),
    "--memory".to_owned(),
    spec.memory_gib.to_string(),
  ]);
  for mount in &spec.mounts {
    words.push("--mount-only".to_owned());
    words.push(csv(&format!("{}:w", mount.to_string_lossy())));
  }
  words
}

/// A string-list flag value as CSV, which is how Lima's flag parser reads it: a
/// comma in a path would otherwise split it into two mounts (checked against
/// Lima 2.2.1; spaces and colons are safe).
fn csv(value: &str) -> String {
  if value.contains([',', '"', '\n', '\r']) {
    format!("\"{}\"", value.replace('"', "\"\""))
  } else {
    value.to_owned()
  }
}

/// Run `command` in VM `name`, starting in `workdir` (a guest path).
pub(crate) fn shell(program: &Path, name: &str, workdir: &Path, command: &[String]) -> Vec<String> {
  let mut words = argv(program, &["shell", "--workdir"]);
  words.push(workdir.to_string_lossy().into_owned());
  words.push(name.to_owned());
  words.push("--".to_owned());
  words.extend(command.iter().cloned());
  words
}

/// `env` with the guest's temporary directory on its own disk and, with
/// `target`, a cargo target directory for project `slug` there too.
pub(crate) fn guest_env(slug: &str, target: bool) -> Vec<String> {
  let mut words = vec!["env".to_owned(), format!("TMPDIR={GUEST_CACHE}/tmp")];
  if target {
    words.push(format!("CARGO_TARGET_DIR={GUEST_CACHE}/target/{slug}"));
  }
  words
}

#[cfg(test)]
#[path = "tests/lima_argv_test.rs"]
mod tests;
