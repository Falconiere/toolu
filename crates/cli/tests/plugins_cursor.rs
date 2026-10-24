//! `toolu plugins --host cursor` (#589): the real binary and real git, on a toolu
//! checkout seeded from a local repository so no test reaches GitHub. Cursor has no
//! plugin command, so no stub host is needed: the plugins land in
//! `<home>/.cursor/plugins/local`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

type Res<T> = Result<T, Box<dyn std::error::Error>>;

fn text(bytes: &[u8]) -> String {
  String::from_utf8_lossy(bytes).into_owned()
}

fn copy(from: &Path, to: &Path) -> Res<()> {
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    let target = to.join(entry.file_name());
    if entry.path().is_dir() {
      copy(&entry.path(), &target)?;
    } else {
      std::fs::copy(entry.path(), target)?;
    }
  }
  Ok(())
}

struct Sb {
  dir: tempfile::TempDir,
}

impl Sb {
  fn new() -> Res<Sb> {
    let dir = tempfile::Builder::new()
      .prefix("toolu-plugins-cursor-")
      .tempdir()?;
    std::fs::create_dir_all(dir.path().join("home"))?;
    Ok(Sb { dir })
  }

  fn home(&self) -> PathBuf {
    self.dir.path().join("home")
  }

  fn git(&self, args: &[&str]) -> Res<()> {
    let out = Command::new("git")
      .args(["-c", "user.name=t", "-c", "user.email=t@t"])
      .args(args)
      .env("HOME", self.home())
      .output()?;
    if out.status.success() {
      Ok(())
    } else {
      Err(text(&out.stderr).into())
    }
  }

  /// The checkout at this binary's tag: a clone of a local repository holding real
  /// copies of two plugins, whose origin URL is the real one.
  fn seed_checkout(&self) -> Res<()> {
    let seed = self.dir.path().join("seed");
    let plugins = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../plugins");
    for name in ["toolu", "ast-grep"] {
      copy(&plugins.join(name), &seed.join("plugins").join(name))?;
    }
    std::fs::create_dir_all(seed.join(".agents/plugins"))?;
    std::fs::write(seed.join(".agents/plugins/marketplace.json"), "{}")?;
    let (seed, dir) = (
      seed.display().to_string(),
      self.checkout().display().to_string(),
    );
    let tag = format!("v{}", env!("CARGO_PKG_VERSION"));
    self.git(&["init", "--quiet", &seed])?;
    self.git(&["-C", &seed, "add", "--all"])?;
    self.git(&["-C", &seed, "commit", "--quiet", "-m", "seed"])?;
    self.git(&["-C", &seed, "tag", &tag])?;
    self.git(&["clone", "--quiet", &seed, &dir])?;
    let real = "https://github.com/Falconiere/toolu.git";
    self.git(&["-C", &dir, "remote", "set-url", "origin", real])
  }

  fn checkout(&self) -> PathBuf {
    self.home().join(".local/share/toolu/cursor-marketplace")
  }

  fn run(&self, args: &[&str]) -> Res<Output> {
    let mut command = Command::new(TOOLU);
    command
      .args(args)
      .current_dir(self.dir.path())
      .env_clear()
      .env("PATH", "/usr/bin:/bin")
      .env("HOME", self.home());
    if let Ok(profile) = std::env::var("LLVM_PROFILE_FILE") {
      command.env("LLVM_PROFILE_FILE", profile);
    }
    Ok(command.output()?)
  }
}

#[test]
fn install_list_update_and_remove_work_without_cursor_agent_on_path() {
  let sb = Sb::new().unwrap();
  sb.seed_checkout().unwrap();
  let out = sb
    .run(&[
      "plugins", "install", "toolu", "ast-grep", "--host", "cursor",
    ])
    .unwrap();
  assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
  let stdout = text(&out.stdout);
  assert!(
    stdout.contains("+ toolu            installed at"),
    "{stdout}"
  );
  let local = sb.home().join(".cursor/plugins/local");
  assert!(local.join("toolu/.toolu-install.json").is_file());
  assert!(local.join("ast-grep/hooks/hooks.json").is_file());

  let listed = sb.run(&["plugins", "list", "--host", "cursor"]).unwrap();
  let listing = text(&listed.stdout);
  assert!(listing.contains("  = toolu            "), "{listing}");
  assert!(
    listing.contains("  - jev              not installed"),
    "{listing}"
  );

  let doctor = sb.run(&["--host", "cursor", "doctor"]).unwrap();
  let report = text(&doctor.stdout);
  assert!(report.contains("ok plugins: 2 toolu plugins"), "{report}");

  let update = sb.run(&["plugins", "update", "--host", "cursor"]).unwrap();
  assert!(text(&update.stdout).contains("= toolu            current at"));

  let gone = sb
    .run(&["plugins", "remove", "toolu", "--yes", "--host", "cursor"])
    .unwrap();
  assert_eq!(gone.status.code(), Some(0), "{}", text(&gone.stderr));
  assert!(!local.join("toolu").exists() && local.join("ast-grep").is_dir());
}

#[test]
fn a_scope_is_refused_and_a_dry_run_touches_nothing() {
  let sb = Sb::new().unwrap();
  let scoped = sb
    .run(&[
      "plugins", "install", "toolu", "--host", "cursor", "--scope", "user",
    ])
    .unwrap();
  assert_eq!(scoped.status.code(), Some(64), "{}", text(&scoped.stderr));
  assert!(text(&scoped.stderr).contains("has no scope concept"));
  let local = sb.home().join(".cursor/plugins/local");
  let missing = sb
    .run(&[
      "plugins",
      "install",
      "toolu",
      "--host",
      "cursor",
      "--dry-run",
    ])
    .unwrap();
  assert_eq!(missing.status.code(), Some(0), "{}", text(&missing.stderr));
  assert!(!local.exists(), "a dry run copies nothing");
}
