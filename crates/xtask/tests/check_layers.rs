//! `cargo xtask check-layers` against real fixture workspaces: the real xtask
//! binary, real `cargo metadata`, temp directories written per test.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

type TestResult = Result<(), Box<dyn Error>>;

/// One fixture crate: its directory under the fixture root, package name,
/// dependencies as `(manifest line under [dependencies] or [dev-dependencies])`, and
/// whether it is a binary.
struct Crate {
  dir: &'static str,
  name: &'static str,
  deps: Vec<String>,
  dev_deps: Vec<String>,
  bin: bool,
}

fn krate(dir: &'static str, name: &'static str) -> Crate {
  Crate {
    dir,
    name,
    deps: Vec::new(),
    dev_deps: Vec::new(),
    bin: false,
  }
}

impl Crate {
  fn dep(mut self, name: &str, dir: &str) -> Self {
    self
      .deps
      .push(format!("{name} = {{ path = \"{}\" }}", up(self.dir, dir)));
    self
  }

  fn dev_dep(mut self, name: &str, dir: &str) -> Self {
    self
      .dev_deps
      .push(format!("{name} = {{ path = \"{}\" }}", up(self.dir, dir)));
    self
  }

  fn renamed_dep(mut self, alias: &str, package: &str, dir: &str) -> Self {
    let path = up(self.dir, dir);
    self.deps.push(format!(
      "{alias} = {{ package = \"{package}\", path = \"{path}\" }}"
    ));
    self
  }

  fn bin(mut self) -> Self {
    self.bin = true;
    self
  }
}

/// `to` relative to `from`, both relative to the fixture root.
fn up(from: &str, to: &str) -> String {
  let depth = Path::new(from).components().count();
  format!("{}{to}", "../".repeat(depth))
}

struct Fixture {
  _temp: tempfile::TempDir,
  root: PathBuf,
}

/// A workspace with `members` listed and `extra` crates written but not listed.
fn workspace(members: Vec<Crate>, extra: Vec<Crate>) -> Result<Fixture, Box<dyn Error>> {
  let temp = tempfile::tempdir()?;
  let root = fs::canonicalize(temp.path())?;
  let list: Vec<String> = members.iter().map(|c| format!("\"{}\"", c.dir)).collect();
  fs::write(
    root.join("Cargo.toml"),
    format!(
      "[workspace]\nresolver = \"3\"\nmembers = [{}]\n",
      list.join(", ")
    ),
  )?;
  for krate in members.iter().chain(extra.iter()) {
    write_crate(&root, krate)?;
  }
  Ok(Fixture { _temp: temp, root })
}

fn write_crate(root: &Path, krate: &Crate) -> TestResult {
  let dir = root.join(krate.dir);
  fs::create_dir_all(dir.join("src"))?;
  let manifest = format!(
    "[package]\nname = \"{}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{}\n\n[dev-dependencies]\n{}\n",
    krate.name,
    krate.deps.join("\n"),
    krate.dev_deps.join("\n")
  );
  fs::write(dir.join("Cargo.toml"), manifest)?;
  let (file, body) = if krate.bin {
    ("main.rs", "fn main() {}\n")
  } else {
    ("lib.rs", "")
  };
  fs::write(dir.join("src").join(file), body)?;
  Ok(())
}

fn xtask(args: &[&str]) -> Result<Output, Box<dyn Error>> {
  Ok(
    Command::new(env!("CARGO_BIN_EXE_xtask"))
      .args(args)
      .output()?,
  )
}

fn check(fixture: &Fixture) -> Result<Output, Box<dyn Error>> {
  let manifest = fixture.root.join("Cargo.toml");
  let manifest = manifest.to_str().ok_or("fixture path is not UTF-8")?;
  xtask(&["check-layers", "--manifest-path", manifest])
}

fn stderr(output: &Output) -> String {
  String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
  String::from_utf8_lossy(&output.stdout).into_owned()
}

fn protocol() -> Crate {
  krate("crates/core/protocol", "toolu-protocol")
}

fn engine() -> Crate {
  krate("crates/core/engine", "toolu-engine")
}

#[test]
fn the_real_workspace_passes() -> TestResult {
  let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml");
  let output = xtask(&["check-layers", "--manifest-path", manifest])?;
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert!(
    stdout(&output).starts_with("check-layers: 7 crates, "),
    "{}",
    stdout(&output)
  );
  Ok(())
}

#[test]
fn runtime_depending_on_engine_fails_and_names_the_edge() -> TestResult {
  let runtime =
    krate("crates/core/runtime", "toolu-runtime").dep("toolu-engine", "crates/core/engine");
  let output = check(&workspace(vec![protocol(), runtime, engine()], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/core/runtime (toolu-runtime, core layer 1) depends on toolu-engine \
     (crates/core/engine, core layer 3): a core crate may depend only on lower core layers\n"
  );
  Ok(())
}

#[test]
fn a_renamed_dependency_is_still_judged() -> TestResult {
  let runtime = krate("crates/core/runtime", "toolu-runtime").renamed_dep(
    "engine",
    "toolu-engine",
    "crates/core/engine",
  );
  let output = check(&workspace(vec![runtime, engine()], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).contains("depends on toolu-engine (crates/core/engine"),
    "{}",
    stderr(&output)
  );
  Ok(())
}

#[test]
fn a_same_layer_core_dependency_fails() -> TestResult {
  let shell = krate("crates/core/shell", "toolu-shell").dep("toolu-http", "crates/core/http");
  let http = krate("crates/core/http", "toolu-http");
  let output = check(&workspace(vec![shell, http], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("(toolu-shell, core layer 2) depends on toolu-http"));
  Ok(())
}

#[test]
fn a_core_crate_depending_on_a_plugin_fails() -> TestResult {
  let protocol = protocol().dep("toolu-statusline", "crates/statusline");
  let statusline = krate("crates/statusline", "toolu-statusline");
  let output = check(&workspace(vec![protocol, statusline], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).contains("(crates/statusline, plugin crate): a core crate may depend only")
  );
  Ok(())
}

#[test]
fn lower_layers_and_dev_dependencies_pass() -> TestResult {
  let runtime = krate("crates/core/runtime", "toolu-runtime")
    .dep("toolu-protocol", "crates/core/protocol")
    .dev_dep("toolu-engine", "crates/core/engine");
  let engine = engine().dep("toolu-runtime", "crates/core/runtime");
  let output = check(&workspace(vec![protocol(), runtime, engine], vec![])?)?;
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(stdout(&output), "check-layers: 3 crates, 2 edges, ok\n");
  Ok(())
}

#[test]
fn a_plugin_depending_on_another_plugin_fails() -> TestResult {
  let statusline =
    krate("crates/statusline", "toolu-statusline").dep("toolu-jev-plugin", "crates/jev");
  let jev = krate("crates/jev", "toolu-jev-plugin");
  let output = check(&workspace(vec![statusline, jev], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).ends_with("a plugin crate may not depend on another plugin crate\n"));
  Ok(())
}

#[test]
fn only_the_hub_may_depend_on_a_rule_crate() -> TestResult {
  let statusline =
    krate("crates/statusline", "toolu-statusline").dep("toolu-ts-quality", "crates/ts-quality");
  let rule = krate("crates/ts-quality", "toolu-ts-quality");
  let output = check(&workspace(vec![statusline, rule], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("(crates/ts-quality, rule crate): only the hub crate"));
  Ok(())
}

#[test]
fn the_hub_links_rules_and_the_cli_builds_the_binary() -> TestResult {
  let rule =
    krate("crates/ts-quality", "toolu-ts-quality").dep("toolu-protocol", "crates/core/protocol");
  let hub = krate("crates/toolu", "toolu-plugin").dep("toolu-ts-quality", "crates/ts-quality");
  let cli = krate("crates/cli", "toolu")
    .dep("toolu-plugin", "crates/toolu")
    .bin();
  let output = check(&workspace(vec![protocol(), rule, hub, cli], vec![])?)?;
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(stdout(&output), "check-layers: 4 crates, 3 edges, ok\n");
  Ok(())
}

#[test]
fn a_plugin_crate_building_a_binary_fails() -> TestResult {
  let statusline = krate("crates/statusline", "toolu-statusline").bin();
  let output = check(&workspace(vec![statusline], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/statusline (toolu-statusline) builds a binary: only crates/cli builds \
     one (crates/xtask excepted)\n"
  );
  Ok(())
}

#[test]
fn nothing_may_depend_on_xtask_or_the_cli() -> TestResult {
  let statusline = krate("crates/statusline", "toolu-statusline")
    .dep("xtask", "crates/xtask")
    .dep("toolu", "crates/cli");
  let xtask = krate("crates/xtask", "xtask").bin();
  let cli = krate("crates/cli", "toolu").bin();
  let output = check(&workspace(vec![statusline, xtask, cli], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  let text = stderr(&output);
  assert!(
    text.contains("tooling crate): nothing may depend on the tooling crate"),
    "{text}"
  );
  assert!(
    text.contains("cli crate): nothing may depend on the cli crate"),
    "{text}"
  );
  Ok(())
}

#[test]
fn a_crate_directory_missing_from_members_fails() -> TestResult {
  let stray = krate("crates/stray", "toolu-stray");
  let output = check(&workspace(vec![protocol()], vec![stray])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/stray has a Cargo.toml but is not a workspace member\n"
  );
  Ok(())
}

#[test]
fn a_core_crate_missing_from_the_layer_table_fails() -> TestResult {
  let extra = krate("crates/core/extra", "toolu-extra");
  let output = check(&workspace(vec![extra], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/core/extra (toolu-extra): crates/core/extra is a core crate with no \
     layer in layers.json\n"
  );
  Ok(())
}

#[test]
fn a_member_outside_crates_fails() -> TestResult {
  let tool = krate("tools/helper", "helper");
  let output = check(&workspace(vec![tool], vec![])?)?;
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("is not a crates/<name> or crates/core/<name> directory"));
  Ok(())
}

#[test]
fn an_invalid_layer_table_exits_2() -> TestResult {
  let fixture = workspace(vec![protocol()], vec![])?;
  let layers = fixture.root.join("layers.json");
  fs::write(
    &layers,
    r#"{"core": [], "rules": [], "hub": "toolu", "binary": "cli", "tooling": "xtask", "extra": 1}"#,
  )?;
  let manifest = fixture.root.join("Cargo.toml");
  let output = xtask(&[
    "check-layers",
    "--manifest-path",
    manifest.to_str().ok_or("not UTF-8")?,
    "--layers",
    layers.to_str().ok_or("not UTF-8")?,
  ])?;
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).contains("invalid layer table: unknown field `extra`"),
    "{}",
    stderr(&output)
  );
  Ok(())
}

#[test]
fn a_failing_cargo_metadata_exits_2() -> TestResult {
  let fixture = workspace(vec![protocol()], vec![])?;
  let missing = fixture.root.join("missing/Cargo.toml");
  let output = xtask(&[
    "check-layers",
    "--manifest-path",
    missing.to_str().ok_or("not UTF-8")?,
  ])?;
  assert_eq!(output.status.code(), Some(2));
  assert!(
    stderr(&output).starts_with("xtask: cargo metadata failed"),
    "{}",
    stderr(&output)
  );
  Ok(())
}

#[test]
fn usage_errors_exit_2() -> TestResult {
  for args in [
    &[][..],
    &["nope"][..],
    &["check-layers", "--layers"][..],
    &["check-layers", "--x", "y"][..],
  ] {
    let output = xtask(args)?;
    assert_eq!(output.status.code(), Some(2), "{args:?}");
    assert!(
      stderr(&output).contains("usage: cargo xtask check-layers"),
      "{args:?}"
    );
  }
  Ok(())
}
