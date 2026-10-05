//! `cargo xtask check-layers` against real temp workspaces: the real xtask
//! binary, real `cargo metadata`, one dependency edge per case.

#[path = "helpers/layered.rs"]
mod layered;

use layered::{Crate, check, krate, stderr, stdout, up, workspace};

/// Builders only the edge cases use.
impl Crate {
  /// Depend on `name` at `dir` for tests only.
  pub(crate) fn dev_dep(mut self, name: &str, dir: &str) -> Self {
    self
      .dev_deps
      .push(format!("{name} = {{ path = \"{}\" }}", up(self.dir, dir)));
    self
  }

  /// Depend on `package` at `dir` under the name `alias`.
  pub(crate) fn renamed_dep(mut self, alias: &str, package: &str, dir: &str) -> Self {
    let path = up(self.dir, dir);
    self.deps.push(format!(
      "{alias} = {{ package = \"{package}\", path = \"{path}\" }}"
    ));
    self
  }

  /// Build a binary instead of a library.
  pub(crate) fn bin(mut self) -> Self {
    self.bin = true;
    self
  }
}

fn protocol() -> Crate {
  krate("crates/core/protocol", "toolu-protocol")
}

fn engine() -> Crate {
  krate("crates/core/engine", "toolu-engine")
}

#[test]
fn runtime_depending_on_engine_fails_and_names_the_edge() {
  let runtime =
    krate("crates/core/runtime", "toolu-runtime").dep("toolu-engine", "crates/core/engine");
  let output = check(&workspace(&[protocol(), runtime, engine()], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/core/runtime (toolu-runtime, core layer 1) depends on toolu-engine \
     (crates/core/engine, core layer 3): a core crate may depend only on lower core layers\n"
  );
}

#[test]
fn a_renamed_dependency_is_still_judged() {
  let runtime = krate("crates/core/runtime", "toolu-runtime").renamed_dep(
    "engine",
    "toolu-engine",
    "crates/core/engine",
  );
  let output = check(&workspace(&[runtime, engine()], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).contains("depends on toolu-engine (crates/core/engine"),
    "{}",
    stderr(&output)
  );
}

#[test]
fn a_same_layer_core_dependency_fails() {
  let shell = krate("crates/core/shell", "toolu-shell").dep("toolu-http", "crates/core/http");
  let http = krate("crates/core/http", "toolu-http");
  let output = check(&workspace(&[shell, http], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("(toolu-shell, core layer 2) depends on toolu-http"));
}

#[test]
fn a_core_crate_depending_on_a_plugin_fails() {
  let protocol = protocol().dep("toolu-statusline", "crates/statusline");
  let statusline = krate("crates/statusline", "toolu-statusline");
  let output = check(&workspace(&[protocol, statusline], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(
    stderr(&output).contains("(crates/statusline, plugin crate): a core crate may depend only")
  );
}

#[test]
fn lower_layers_and_dev_dependencies_pass() {
  let runtime = krate("crates/core/runtime", "toolu-runtime")
    .dep("toolu-protocol", "crates/core/protocol")
    .dev_dep("toolu-engine", "crates/core/engine");
  let engine = engine().dep("toolu-runtime", "crates/core/runtime");
  let output = check(&workspace(&[protocol(), runtime, engine], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(stdout(&output), "check-layers: 3 crates, 2 edges, ok\n");
}

#[test]
fn a_plugin_depending_on_another_plugin_fails() {
  let statusline =
    krate("crates/statusline", "toolu-statusline").dep("toolu-jev-plugin", "crates/jev");
  let jev = krate("crates/jev", "toolu-jev-plugin");
  let output = check(&workspace(&[statusline, jev], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).ends_with("a plugin crate may not depend on another plugin crate\n"));
}

#[test]
fn only_the_hub_may_depend_on_a_rule_crate() {
  let statusline =
    krate("crates/statusline", "toolu-statusline").dep("toolu-ts-quality", "crates/ts-quality");
  let rule = krate("crates/ts-quality", "toolu-ts-quality");
  let output = check(&workspace(&[statusline, rule], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert!(stderr(&output).contains("(crates/ts-quality, rule crate): only the hub crate"));
}

#[test]
fn the_hub_links_rules_and_the_cli_builds_the_binary() {
  let rule =
    krate("crates/ts-quality", "toolu-ts-quality").dep("toolu-protocol", "crates/core/protocol");
  let hub = krate("crates/toolu", "toolu-plugin").dep("toolu-ts-quality", "crates/ts-quality");
  let cli = krate("crates/cli", "toolu")
    .dep("toolu-plugin", "crates/toolu")
    .bin();
  let output = check(&workspace(&[protocol(), rule, hub, cli], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
  assert_eq!(stdout(&output), "check-layers: 4 crates, 3 edges, ok\n");
}

#[test]
fn the_cli_may_not_depend_on_a_rule_crate() {
  let rule = krate("crates/ts-quality", "toolu-ts-quality");
  let cli = krate("crates/cli", "toolu")
    .dep("toolu-ts-quality", "crates/ts-quality")
    .bin();
  let output = check(&workspace(&[rule, cli], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/cli (toolu, cli crate) depends on toolu-ts-quality (crates/ts-quality, \
     rule crate): only the hub crate (crates/toolu) may depend on a rule crate\n"
  );
}

#[test]
fn a_plugin_crate_building_a_binary_fails() {
  let statusline = krate("crates/statusline", "toolu-statusline").bin();
  let output = check(&workspace(&[statusline], &[]).unwrap()).unwrap();
  assert_eq!(output.status.code(), Some(1));
  assert_eq!(
    stderr(&output),
    "check-layers: crates/statusline (toolu-statusline) builds a binary: only crates/cli builds \
     one (crates/xtask excepted)\n"
  );
}

#[test]
fn nothing_may_depend_on_xtask_or_the_cli() {
  let statusline = krate("crates/statusline", "toolu-statusline")
    .dep("xtask", "crates/xtask")
    .dep("toolu", "crates/cli");
  let xtask = krate("crates/xtask", "xtask").bin();
  let cli = krate("crates/cli", "toolu").bin();
  let output = check(&workspace(&[statusline, xtask, cli], &[]).unwrap()).unwrap();
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
}
