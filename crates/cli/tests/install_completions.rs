//! `install.sh` completion files. The script is sourced, so the test uses the
//! built `toolu` and does not download a release.

use std::error::Error;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type Res<T> = Result<T, Box<dyn Error>>;

fn installer() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../install.sh")
}

fn bash(body: &str, install_dir: &Path) -> Res<Output> {
  Ok(
    Command::new("bash")
      .arg("-c")
      .arg(body)
      .arg("bash")
      .arg(installer())
      .arg(install_dir)
      .output()?,
  )
}

fn completion_files(prefix: &Path) -> [PathBuf; 3] {
  [
    prefix.join("share/bash-completion/completions/toolu"),
    prefix.join("share/zsh/site-functions/_toolu"),
    prefix.join("share/fish/vendor_completions.d/toolu.fish"),
  ]
}

fn install_toolu(dir: &Path) -> Res<()> {
  std::fs::create_dir_all(dir)?;
  let binary = dir.join("toolu");
  std::fs::copy(env!("CARGO_BIN_EXE_toolu"), &binary)?;
  let mut perms = std::fs::metadata(&binary)?.permissions();
  perms.set_mode(0o755);
  std::fs::set_permissions(binary, perms)?;
  Ok(())
}

fn streams(output: &Output) -> Res<(String, String)> {
  Ok((
    String::from_utf8(output.stdout.clone())?,
    String::from_utf8(output.stderr.clone())?,
  ))
}

#[test]
fn install_writes_three_scripts_and_uninstall_removes_them() {
  let dir = tempfile::tempdir().unwrap();
  let install_dir = dir.path().join("bin");
  install_toolu(&install_dir).unwrap();
  let wrote = bash(
    "source \"$1\"; INSTALL_DIR=\"$2\"; install_completions",
    &install_dir,
  )
  .unwrap();
  let (stdout, stderr) = streams(&wrote).unwrap();
  assert_eq!(wrote.status.code(), Some(0), "{stderr}");
  let fpath = dir.path().join("share/zsh/site-functions");
  assert!(
    stdout.contains(&format!(
      "zsh: add {} to fpath before compinit",
      fpath.display()
    )),
    "{stdout}"
  );
  for path in completion_files(dir.path()) {
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("plugins"), "{path:?}");
    let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "{path:?}");
  }
  assert!(install_dir.join("toolu").is_file());

  let removed = bash("source \"$1\"; INSTALL_DIR=\"$2\"; uninstall", &install_dir).unwrap();
  let (stdout, stderr) = streams(&removed).unwrap();
  assert_eq!(removed.status.code(), Some(0), "{stderr}{stdout}");
  for path in completion_files(dir.path()) {
    assert!(!path.exists(), "{path:?}");
  }
  assert!(!install_dir.join("toolu").exists());
}

#[test]
fn check_prints_completion_paths_and_writes_nothing() {
  let dir = tempfile::tempdir().unwrap();
  let install_dir = dir.path().join("bin");
  std::fs::create_dir(&install_dir).unwrap();
  let output = bash(
    "source \"$1\"; INSTALL_DIR=\"$2\"; detect_platform; print_plan",
    &install_dir,
  )
  .unwrap();
  let (stdout, stderr) = streams(&output).unwrap();
  assert_eq!(output.status.code(), Some(0), "{stderr}");
  let bash_completion = completion_files(dir.path())[0].display().to_string();
  assert!(stdout.contains(&bash_completion), "{stdout}");
  assert!(!dir.path().join("share").exists());
}

#[test]
fn uninstall_refuses_a_homebrew_cellar_link_before_removing_completions() {
  let dir = tempfile::tempdir().unwrap();
  let install_dir = dir.path().join("bin");
  std::fs::create_dir_all(dir.path().join("Cellar/toolu/1")).unwrap();
  std::fs::create_dir(&install_dir).unwrap();
  symlink(
    dir.path().join("Cellar/toolu/1/toolu"),
    install_dir.join("toolu"),
  )
  .unwrap();
  let script = completion_files(dir.path())[0].clone();
  std::fs::create_dir_all(script.parent().unwrap()).unwrap();
  std::fs::write(&script, "keep").unwrap();
  let output = bash("source \"$1\"; INSTALL_DIR=\"$2\"; uninstall", &install_dir).unwrap();
  let (stdout, stderr) = streams(&output).unwrap();
  assert_eq!(output.status.code(), Some(1), "{stdout}{stderr}");
  assert!(stderr.contains("Homebrew"), "{stderr}");
  assert_eq!(std::fs::read_to_string(&script).unwrap(), "keep");
  assert!(
    install_dir
      .join("toolu")
      .symlink_metadata()
      .unwrap()
      .file_type()
      .is_symlink()
  );
}
