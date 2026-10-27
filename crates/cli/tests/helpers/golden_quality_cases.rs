//! The quality case files (`fixtures/quality/<plugin>.json`) and their goldens
//! (`<plugin>-golden.json`), parsed strictly, with the golden key of each case.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::Res;

/// The host a case runs as.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Host {
  /// Claude Code.
  Claude,
  /// Codex, where every edit is an `apply_patch`.
  Codex,
}

impl Host {
  /// The name used in golden keys and the sandbox config directory.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Host::Claude => "claude",
      Host::Codex => "codex",
    }
  }
}

/// One file of an `apply_patch` step.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PatchFile {
  /// `add`, `update` or `delete`.
  pub(crate) op: String,
  /// The patched path, as written in the patch.
  pub(crate) path: String,
  /// Added lines, or raw diff lines for an update.
  #[serde(default)]
  pub(crate) lines: Vec<String>,
}

/// One tool call of a case.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Step {
  /// Files written before the call.
  #[serde(default)]
  pub(crate) write: BTreeMap<String, String>,
  /// Files removed before the call.
  #[serde(default)]
  pub(crate) remove: Vec<String>,
  /// `Write` (default), `Edit`, `MultiEdit`, `Bash` or `Delete`.
  pub(crate) tool: Option<String>,
  /// The file the call touched.
  pub(crate) file: Option<String>,
  /// Pass `file` as written instead of as an absolute project path.
  #[serde(default)]
  pub(crate) relative: bool,
  /// Extra `tool_input` fields.
  #[serde(default)]
  pub(crate) input: Map<String, Value>,
  /// A Codex patch built from these files.
  pub(crate) patch: Option<Vec<PatchFile>>,
  /// A Codex patch given verbatim.
  pub(crate) raw_patch: Option<String>,
  /// Environment for this call only.
  #[serde(default)]
  pub(crate) env: BTreeMap<String, String>,
}

/// The small environment variant a case runs its steps in.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum CaseEnv {
  /// `PATH` is a directory of links to everything on `PATH` but `without`.
  PathWithout {
    /// Programs the case must not find.
    without: Vec<String>,
  },
  /// An `ast-grep` script first on `PATH`.
  StubAstGrep {
    /// The script body after the shebang.
    body: String,
  },
  /// `TOOLU_SETTINGS_DIR` names a project `settings/` folder with these files.
  ProjectSettings {
    /// File name to body.
    files: BTreeMap<String, String>,
  },
}

/// A path-like value that is either text or a `{"$path": ...}` token.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(crate) enum Tagged {
  /// Literal text.
  Text(String),
  /// A path that may start with a sandbox token such as `$PROJECT`.
  Path {
    /// The path.
    #[serde(rename = "$path")]
    path: String,
  },
}

/// One sandbox mutation before the steps. Operations the fixtures never use
/// fail to parse, so a new one is a loud error.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Action {
  /// Write a file.
  Write {
    /// Where.
    path: String,
    /// What.
    body: String,
  },
  /// Remove a file or tree.
  Remove {
    /// Where.
    path: String,
  },
  /// Create a directory.
  Mkdir {
    /// Where.
    path: String,
  },
  /// Create a symbolic link.
  Symlink {
    /// The link.
    path: String,
    /// What it points to.
    target: Tagged,
  },
  /// Change a file mode.
  Chmod {
    /// Where.
    path: String,
    /// Octal mode text.
    mode: String,
  },
  /// Run git in the project.
  Git {
    /// The arguments.
    args: Vec<Tagged>,
  },
}

/// A named deviation: the port does something the capture did not.
#[derive(Debug, Deserialize)]
pub(crate) struct Deviation {
  /// Text the last step's stdout must contain.
  pub(crate) contains: Vec<String>,
}

/// One golden case.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Case {
  /// The unique case name.
  pub(crate) name: String,
  /// The bats titles the case ports.
  #[serde(default)]
  pub(crate) from: Vec<String>,
  /// The hosts it runs on; Claude when absent.
  pub(crate) hosts: Option<Vec<Host>>,
  /// Files in the project, committed.
  pub(crate) project: BTreeMap<String, String>,
  /// More committed files.
  #[serde(default)]
  pub(crate) commit: BTreeMap<String, String>,
  /// The project `toolu.config.json`.
  pub(crate) config: Option<Value>,
  /// More plugins to register.
  #[serde(default)]
  pub(crate) register: Vec<String>,
  /// The environment variant.
  pub(crate) env: Option<CaseEnv>,
  /// Mutations before the steps.
  #[serde(default)]
  pub(crate) setup: Vec<Action>,
  /// The tool calls.
  pub(crate) steps: Vec<Step>,
  /// The decision class of the last step.
  pub(crate) expect: String,
  /// Text the last step's stdout contains.
  #[serde(default)]
  pub(crate) contains: Vec<String>,
  /// Text it does not.
  #[serde(default)]
  pub(crate) absent: Vec<String>,
  /// Compare line sets, not line order.
  #[serde(default)]
  pub(crate) unordered: bool,
  /// A named deviation from the capture.
  pub(crate) deviation: Option<Deviation>,
}

impl Case {
  /// The hosts the case runs on.
  pub(crate) fn hosts(&self) -> Vec<Host> {
    self.hosts.clone().unwrap_or_else(|| vec![Host::Claude])
  }

  /// Whether the case links a Node tool into the project's `node_modules/.bin`,
  /// which the hook runs under its own timeout.
  pub(crate) fn links_a_node_tool(&self) -> bool {
    self.setup.iter().any(|action| {
      matches!(action, Action::Symlink { path, .. } if path.contains("/node_modules/.bin/"))
    })
  }
}

/// A case file: a version and the cases.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile {
  version: u32,
  cases: Vec<Case>,
}

/// What one step printed and left behind, normalised.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct Recorded {
  /// Standard output.
  pub(crate) stdout: String,
  /// Standard error.
  pub(crate) stderr: String,
  /// The exit status.
  pub(crate) exit_code: i32,
  /// Files under the project's `.claude/` and `.codex/`.
  pub(crate) state: BTreeMap<String, String>,
}

/// A golden capture.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Golden {
  /// The commit the bash module was captured at.
  pub(crate) base: String,
  /// Steps by `"<case> [<host>]"`.
  pub(crate) cases: BTreeMap<String, Vec<Recorded>>,
  /// Every bats title at the base (the Rust golden only).
  pub(crate) bats_tests: Option<Vec<String>>,
}

/// The golden key of `case` on `host`.
pub(crate) fn key(case: &Case, host: Host) -> String {
  format!("{} [{}]", case.name, host.name())
}

fn dir() -> PathBuf {
  PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/quality")
}

/// The cases of `fixtures/quality/<name>.json`, version and names checked.
pub(crate) fn load_cases(name: &str) -> Res<Vec<Case>> {
  let path = dir().join(format!("{name}.json"));
  let file: CaseFile = serde_json::from_str(&std::fs::read_to_string(&path)?)
    .map_err(|err| format!("{}: {err}", path.display()))?;
  if file.version != 1 {
    return Err(format!("{}: unsupported version {}", path.display(), file.version).into());
  }
  let mut seen = BTreeSet::new();
  for case in &file.cases {
    if !seen.insert(case.name.clone()) {
      return Err(format!("{}: duplicate case name: {}", path.display(), case.name).into());
    }
  }
  Ok(file.cases)
}

/// The golden `fixtures/quality/<name>-golden.json`.
pub(crate) fn load_golden(name: &str) -> Res<Golden> {
  let path = dir().join(format!("{name}-golden.json"));
  let golden = serde_json::from_str(&std::fs::read_to_string(&path)?)
    .map_err(|err| format!("{}: {err}", path.display()))?;
  Ok(golden)
}

/// Bats titles the Rust golden leaves out on purpose
/// (`fixtures/quality/rust-excluded.json`).
pub(crate) fn load_excluded() -> Res<Vec<String>> {
  #[derive(Deserialize)]
  #[serde(deny_unknown_fields)]
  struct Excluded {
    version: u32,
    names: Vec<String>,
  }
  let path = dir().join("rust-excluded.json");
  let excluded: Excluded = serde_json::from_str(&std::fs::read_to_string(&path)?)
    .map_err(|err| format!("{}: {err}", path.display()))?;
  if excluded.version != 1 {
    return Err(format!("{}: unsupported version", path.display()).into());
  }
  Ok(excluded.names)
}
