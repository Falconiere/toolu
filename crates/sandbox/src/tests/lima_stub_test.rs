//! A stub `limactl` for the Lima backend's tests: a real executable in a temp
//! directory that records each call, prints a version and a `list` answer from
//! files beside it, and fails a verb when a `fail-<verb>` file holds an exit
//! code. A context around it has the stub first on `PATH` and its own state home.

use std::io;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use toolu_runtime::env::Env;

use crate::context::Context;
use crate::facts::{Facts, Os};
use crate::project::Project;
use crate::settings::{Backend, Resolved};

const SCRIPT: &str = r#"#!/bin/sh
d=$(dirname "$0")
printf '%s\n' "$*" >> "$d/calls"
case "$1" in
  --version) cat "$d/version"; exit 0 ;;
  list)
    if [ -f "$d/list-first.json" ]; then cat "$d/list-first.json"; rm "$d/list-first.json"; exit 0; fi
    cat "$d/list.json"; exit 0 ;;
esac
verb=$1
[ "$verb" = "--tty=false" ] && verb=$2
if [ -f "$d/fail-$verb" ]; then
  echo "stub $verb failed" >&2
  exit "$(cat "$d/fail-$verb")"
fi
exit 0
"#;

/// Run `program` once so that nothing holds it open for writing any more. A thread
/// that forked while this one wrote the file keeps a copy of the write descriptor
/// until its child execs, and until then `exec` of the file fails with `ETXTBSY`
/// ("text file busy"). Once one run succeeds no such copy is left, and the file
/// is never written again.
fn settle(program: &Path) -> io::Result<()> {
  for _ in 0..500 {
    match Command::new(program).arg("--version").output() {
      Err(err) if err.kind() == io::ErrorKind::ExecutableFileBusy => {
        std::thread::sleep(Duration::from_millis(10));
      }
      other => return other.map(|_| ()),
    }
  }
  Err(io::Error::other("the stub limactl stayed busy"))
}

/// The stub and a project beside it.
pub(crate) struct Stub {
  _dir: tempfile::TempDir,
  /// The canonical temp directory.
  pub(crate) root: PathBuf,
}

impl Stub {
  /// A stub reporting `version`, with no VM and a finished setup.
  pub(crate) fn new(version: &str) -> io::Result<Stub> {
    let dir = tempfile::tempdir()?;
    let root = std::fs::canonicalize(dir.path())?;
    for sub in ["bin", "repo/sub", "state"] {
      std::fs::create_dir_all(root.join(sub))?;
    }
    let program = root.join("bin/limactl");
    std::fs::write(&program, SCRIPT)?;
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))?;
    std::fs::write(
      root.join("bin/version"),
      format!("limactl version {version}\n"),
    )?;
    settle(&program)?;
    let stub = Stub { _dir: dir, root };
    stub.list(None)?;
    let state = stub.root.join("state/toolu/sandbox");
    std::fs::create_dir_all(&state)?;
    std::fs::write(state.join("toolu-sandbox.json"), "{}")?;
    Ok(stub)
  }

  /// Forget that setup finished, as a failed or never-run setup leaves it.
  pub(crate) fn unprovision(&self) -> io::Result<()> {
    std::fs::remove_file(self.root.join("state/toolu/sandbox/toolu-sandbox.json"))
  }

  /// The stub's path.
  pub(crate) fn program(&self) -> PathBuf {
    self.root.join("bin/limactl")
  }

  /// The project root the context runs in.
  pub(crate) fn repo(&self) -> PathBuf {
    self.root.join("repo")
  }

  /// `list` reports VM `toolu-sandbox` as `status` mounting `mounts`, or no VM.
  pub(crate) fn list(&self, vm: Option<(&str, &[&Path])>) -> io::Result<()> {
    std::fs::write(self.root.join("bin/list.json"), listing(vm))
  }

  /// Like [`Stub::list`], for the next `list` call only: a VM another command
  /// then changes.
  pub(crate) fn list_first(&self, vm: Option<(&str, &[&Path])>) -> io::Result<()> {
    std::fs::write(self.root.join("bin/list-first.json"), listing(vm))
  }

  /// Make the next `verb` calls exit with `code`.
  pub(crate) fn fail(&self, verb: &str, code: i32) -> io::Result<()> {
    std::fs::write(self.root.join(format!("bin/fail-{verb}")), code.to_string())
  }

  /// Every call so far, its arguments joined by spaces, without `--version` and `list`.
  pub(crate) fn calls(&self) -> Vec<String> {
    std::fs::read_to_string(self.root.join("bin/calls"))
      .unwrap_or_default()
      .lines()
      .filter(|line| !line.starts_with("--version") && !line.starts_with("list "))
      .map(str::to_owned)
      .collect()
  }

  /// A Lima-backend context in the repo, with the stub on `PATH`.
  pub(crate) fn context(&self) -> Context {
    let path = format!("{}:/usr/bin:/bin", self.root.join("bin").display());
    Context {
      env: Env::from_pairs([
        ("PATH", path.as_str()),
        ("HOME", self.root.to_string_lossy().as_ref()),
        (
          "XDG_STATE_HOME",
          self.root.join("state").to_string_lossy().as_ref(),
        ),
      ]),
      facts: Facts {
        os: Os::Macos,
        cores: 12,
        memory_gib: 32,
      },
      settings: Resolved {
        backend: Backend::Lima,
        name: "toolu-sandbox".to_owned(),
        cpus: 10,
        memory_gib: 16,
        disk_gib: 100,
        template: None,
        mounts: Vec::new(),
      },
      project: Project {
        cwd: self.repo().join("sub"),
        root: Some(self.repo()),
        common: None,
      },
    }
  }
}

/// The `limactl list --format json` text for VM `toolu-sandbox`, or for no VM.
fn listing(vm: Option<(&str, &[&Path])>) -> String {
  vm.map_or_else(String::new, |(status, mounts)| {
    let mounts: Vec<serde_json::Value> = mounts
      .iter()
      .map(|mount| serde_json::json!({ "location": mount, "writable": true }))
      .collect();
    let doc = serde_json::json!({ "name": "toolu-sandbox", "status": status, "config": { "mounts": mounts } });
    format!("{doc}\n")
  })
}
