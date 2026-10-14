//! The npm wrapper (`toolu-plugins`, #438), `tools/toolu-cli/npm/cli.js`, run
//! with the real `node` against the built `toolu` and shell stand-ins: it runs
//! the native binary's `toolu plugins`, never takes itself for that binary,
//! and without one names the install commands instead of downloading anything.

use std::os::unix::fs::{PermissionsExt as _, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The `toolu` binary under test.
const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// A helper's result; the tests unwrap it.
type Res<T> = Result<T, Box<dyn std::error::Error>>;

/// A `PATH` with no `toolu` on it.
const SYSTEM: &str = "/usr/bin:/bin";

const INSTALL: &str = "Install it, then run this again:\n  curl -fsSL \
                       https://get.toolu.sh/pkg/toolu/install | bash\n  brew install \
                       falconiere/tap/toolu\nSee \
                       https://github.com/Falconiere/toolu/blob/main/docs/install.md\n";

fn repo() -> PathBuf {
  Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn wrapper() -> PathBuf {
  repo().join("tools/toolu-cli/npm/cli.js")
}

/// The catalog the wrapper passes from this checkout: the copy `npm pack`'s
/// `prepack` leaves in `assets/`, else the repository's.
fn catalog() -> PathBuf {
  let shipped = repo().join("tools/toolu-cli/npm/assets/marketplace.json");
  let path = if shipped.is_file() {
    shipped
  } else {
    repo().join(".claude-plugin/marketplace.json")
  };
  std::fs::canonicalize(&path).unwrap_or(path)
}

/// The first `node` on this process's `PATH`; these tests need it.
fn node() -> Res<PathBuf> {
  let path = std::env::var_os("PATH").unwrap_or_default();
  std::env::split_paths(&path)
    .map(|dir| dir.join("node"))
    .find(|candidate| candidate.is_file())
    .ok_or_else(|| "node is not on PATH; the npm wrapper tests need it".into())
}

/// An executable `#!/bin/sh` script at `path`.
fn script(path: &Path, body: &str) -> Res<PathBuf> {
  std::fs::create_dir_all(path.parent().ok_or("no parent")?)?;
  std::fs::write(path, format!("#!/bin/sh\n{body}\n"))?;
  std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
  Ok(path.to_path_buf())
}

/// What one wrapper run printed and how it ended.
#[derive(Debug)]
struct Run {
  code: Option<i32>,
  stdout: String,
  stderr: String,
}

/// `node <cli> args` with exactly `env` (plus `TERM`), in `cwd`.
fn run(cli: &Path, args: &[&str], env: &[(&str, &str)], cwd: &Path) -> Res<Run> {
  let output = Command::new(node()?)
    .arg(cli)
    .args(args)
    .env_clear()
    .env("TERM", "dumb")
    .envs(env.iter().copied())
    .current_dir(cwd)
    .output()?;
  Ok(Run {
    code: output.status.code(),
    stdout: String::from_utf8(output.stdout)?,
    stderr: String::from_utf8(output.stderr)?,
  })
}

fn temp() -> Res<(tempfile::TempDir, PathBuf)> {
  let dir = tempfile::tempdir()?;
  let root = std::fs::canonicalize(dir.path())?;
  Ok((dir, root))
}

/// A stand-in native binary that answers the probes and logs every other call.
fn native_stub(path: &Path, log: &Path, tail: &str) -> Res<PathBuf> {
  script(
    path,
    &format!(
      "case \"$1\" in --hook-protocol) echo 2; exit 0 ;; esac\n\
       [ \"$3\" = --help ] && {{ echo \"Usage: toolu plugins install\"; exit 0; }}\n\
       echo \"$*\" >> '{}'\n{tail}",
      log.display()
    ),
  )
}

#[test]
fn the_manifest_declares_toolu_plugins_and_no_toolu_bin() {
  let text = std::fs::read_to_string(repo().join("tools/toolu-cli/npm/package.json")).unwrap();
  let manifest: Value = serde_json::from_str(&text).unwrap();
  assert_eq!(
    manifest["bin"],
    serde_json::json!({ "toolu-plugins": "cli.js" })
  );
  assert_eq!(manifest["files"], serde_json::json!(["cli.js", "assets"]));
}

#[test]
fn a_toolu_bin_that_is_the_wrapper_itself_is_never_run() {
  let (_dir, root) = temp().unwrap();
  let link = root.join("bin/toolu");
  std::fs::create_dir_all(root.join("bin")).unwrap();
  symlink(wrapper(), &link).unwrap();
  let link_text = link.display().to_string();
  let home = root.display().to_string();
  let env = [("PATH", SYSTEM), ("HOME", &home), ("TOOLU_BIN", &link_text)];
  let ran = run(&wrapper(), &["install", "--dry-run"], &env, &root).unwrap();
  assert_eq!(ran.code, Some(69), "{ran:?}");
  assert_eq!(
    ran.stderr,
    format!(
      "toolu-plugins: the native toolu binary was not found (looked at {link_text}).\n{INSTALL}"
    )
  );
}

#[test]
fn a_native_binary_without_plugins_is_too_old() {
  let (_dir, root) = temp().unwrap();
  let old = script(
    &root.join("bin/old"),
    "[ \"$1\" = \"--hook-protocol\" ] && { echo 2; exit 0; }; exit 64",
  )
  .unwrap();
  let old = old.display().to_string();
  let home = root.display().to_string();
  let env = [("PATH", SYSTEM), ("HOME", &home), ("TOOLU_BIN", &old)];
  let ran = run(&wrapper(), &["list"], &env, &root).unwrap();
  assert_eq!(ran.code, Some(69), "{ran:?}");
  assert!(ran.stderr.contains(&format!(
    "{old} has no `toolu plugins`; it predates this package"
  )));
}

#[test]
fn with_the_native_binary_it_runs_toolu_plugins_from_any_directory() {
  let (_dir, root) = temp().unwrap();
  let home = root.display().to_string();
  let env = [("PATH", SYSTEM), ("HOME", &home), ("TOOLU_BIN", TOOLU)];
  for cwd in [repo(), root.clone()] {
    let args = ["install", "--dry-run", "--host", "claude", "jev"];
    let ran = run(&wrapper(), &args, &env, &cwd).unwrap();
    assert_eq!((ran.code, ran.stderr.as_str()), (Some(0), ""), "{ran:?}");
    assert_eq!(
      ran.stdout,
      "Planned commands (nothing was run):\n  claude plugin install jev@toolu\n"
    );
  }
  let usage = run(&wrapper(), &["skills"], &env, &root).unwrap();
  assert_eq!(usage.code, Some(64), "{usage:?}");
  assert!(usage.stderr.contains("skills"));
}

#[test]
fn the_words_the_catalog_the_status_and_a_killing_signal_pass_through() {
  let (_dir, root) = temp().unwrap();
  let log = root.join("argv.log");
  let tail = "case \"$*\" in *\" list \"*) kill -TERM $$ ;; esac\nexit 3";
  let native = native_stub(&root.join("bin/native"), &log, tail).unwrap();
  let native = native.display().to_string();
  let home = root.display().to_string();
  let env = [("PATH", SYSTEM), ("HOME", &home), ("TOOLU_BIN", &native)];
  assert_eq!(
    run(&wrapper(), &["install", "jev"], &env, &root)
      .unwrap()
      .code,
    Some(3)
  );
  let mine = ["install", "--catalog", "/mine.json"];
  assert_eq!(run(&wrapper(), &mine, &env, &root).unwrap().code, Some(3));
  let equals = ["install", "--catalog=/mine.json"];
  assert_eq!(run(&wrapper(), &equals, &env, &root).unwrap().code, Some(3));
  assert_eq!(
    run(&wrapper(), &["--help"], &env, &root).unwrap().code,
    Some(3)
  );
  let killed = run(&wrapper(), &["--host", "codex", "list"], &env, &root).unwrap();
  assert_eq!(killed.code, Some(143), "{killed:?}");
  let catalog = catalog();
  let catalog = catalog.display();
  assert_eq!(
    std::fs::read_to_string(&log).unwrap(),
    format!(
      "plugins install --catalog {catalog} jev\nplugins install --catalog /mine.json\n\
       plugins install --catalog=/mine.json\nplugins --help\n\
       plugins --host codex list --catalog {catalog}\n"
    )
  );
}

#[test]
fn path_order_skips_legacy_scripts_and_probes_each_real_path_once() {
  let (_dir, root) = temp().unwrap();
  script(
    &root.join("legacy/toolu"),
    "echo \"toolu: unknown flag: $1\" >&2; exit 2",
  )
  .unwrap();
  let log = root.join("probes.log");
  let native = script(
    &root.join("native"),
    &format!(
      "echo \"$*\" >> '{}'\n[ \"$1\" = \"--hook-protocol\" ] && echo 2\n\
       [ \"$1\" = plugins ] && echo \"Usage: toolu plugins install\"\nexit 0",
      log.display()
    ),
  )
  .unwrap();
  for dir in ["a", "b"] {
    std::fs::create_dir_all(root.join(dir)).unwrap();
    symlink(&native, root.join(dir).join("toolu")).unwrap();
  }
  let path = format!(
    "{}:{}:{}:{SYSTEM}",
    root.join("legacy").display(),
    root.join("a").display(),
    root.join("b").display()
  );
  let home = root.join("home").display().to_string();
  let ran = run(
    &wrapper(),
    &["list"],
    &[("PATH", &path), ("HOME", &home)],
    &root,
  )
  .unwrap();
  assert_eq!(ran.code, Some(0), "{ran:?}");
  let catalog = catalog();
  assert_eq!(
    std::fs::read_to_string(&log).unwrap(),
    format!(
      "--hook-protocol\nplugins install --help\nplugins list --catalog {}\n",
      catalog.display()
    )
  );
}

#[test]
fn a_bad_toolu_bin_is_not_passed_over_for_a_good_binary_on_path() {
  let (_dir, root) = temp().unwrap();
  std::fs::create_dir_all(root.join("bin")).unwrap();
  symlink(TOOLU, root.join("bin/toolu")).unwrap();
  let typo = root.join("typo").display().to_string();
  let path = format!("{}:{SYSTEM}", root.join("bin").display());
  let home = root.display().to_string();
  let env = [
    ("PATH", path.as_str()),
    ("HOME", &home),
    ("TOOLU_BIN", &typo),
  ];
  let ran = run(&wrapper(), &["list"], &env, &root).unwrap();
  assert_eq!(ran.code, Some(69), "{ran:?}");
  assert!(
    ran.stderr.contains(&format!("(looked at {typo})")),
    "{ran:?}"
  );
}

#[test]
fn a_toolu_inside_the_wrapper_package_or_a_global_install_is_the_wrapper() {
  let (_dir, root) = temp().unwrap();
  let log = root.join("argv.log");
  for package in ["pkg", "lib/node_modules/@toolu/plugins"] {
    let dir = root.join(package);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(wrapper(), dir.join("cli.js")).unwrap();
    std::fs::write(
      dir.join("package.json"),
      "{\"type\":\"module\",\"version\":\"1.2.3\"}",
    )
    .unwrap();
    native_stub(&dir.join("bin/toolu"), &log, "exit 0").unwrap();
    let path = format!("{}:{SYSTEM}", dir.join("bin").display());
    let home = root.join("home").display().to_string();
    let env = [("PATH", path.as_str()), ("HOME", &home)];
    let ran = run(&dir.join("cli.js"), &["list"], &env, &root).unwrap();
    assert_eq!(ran.code, Some(69), "{package}: {ran:?}");
    let version = run(&dir.join("cli.js"), &["--version"], &env, &root).unwrap();
    assert_eq!(version.stdout, "1.2.3\n");
  }
  assert!(!log.exists(), "a toolu inside the package ran");
}

#[test]
fn version_answers_without_a_native_binary() {
  let (_dir, root) = temp().unwrap();
  let text = std::fs::read_to_string(repo().join("tools/toolu-cli/npm/package.json")).unwrap();
  let manifest: Value = serde_json::from_str(&text).unwrap();
  let home = root.display().to_string();
  let ran = run(
    &wrapper(),
    &["--version"],
    &[("PATH", SYSTEM), ("HOME", &home)],
    &root,
  )
  .unwrap();
  assert_eq!(ran.code, Some(0));
  assert_eq!(
    ran.stdout,
    format!("{}\n", manifest["version"].as_str().unwrap())
  );
}
