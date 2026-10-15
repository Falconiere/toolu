//! The case files and golden captures of the pre-tool gate suites
//! (`fixtures/gates`), read the way the deleted TypeScript suites read them.

use std::collections::BTreeMap;
use std::error::Error;
use std::fs;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A helper's result; the tests unwrap it.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// The repository root.
pub(crate) const REPO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

/// The hosts the golden captures cover.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Host {
  /// Claude Code.
  Claude,
  /// Codex.
  Codex,
}

impl Host {
  /// The host's name in a golden key.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Host::Claude => "claude",
      Host::Codex => "codex",
    }
  }

  /// The host's project state directory.
  pub(crate) fn state_dir(self) -> &'static str {
    match self {
      Host::Claude => ".claude",
      Host::Codex => ".codex",
    }
  }
}

/// Where a host config lives.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Scope {
  /// Inside the project.
  Project,
  /// In the user's home.
  User,
}

/// A path that names a sandbox location (`{"$path": "$PROJECT/x"}`).
#[derive(Debug, Deserialize)]
pub(crate) struct PathTag {
  /// The path with its token.
  #[serde(rename = "$path")]
  pub(crate) path: String,
}

/// One step of sandbox setup.
#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "lowercase", deny_unknown_fields)]
pub(crate) enum Action {
  /// Write a file.
  Write {
    /// Where.
    path: String,
    /// A string or a `$template`.
    body: Value,
  },
  /// Remove a path.
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
    target: PathTag,
  },
  /// Change a mode.
  Chmod {
    /// Where.
    path: String,
    /// Three octal digits.
    mode: String,
  },
  /// Run git in the project.
  Git {
    /// Strings, `$path` and `$template` values.
    args: Vec<Value>,
  },
  /// Write a host config.
  Config {
    /// Whose.
    host: Host,
    /// Which one.
    scope: Scope,
    /// The document.
    body: Value,
  },
}

/// A case of the A, B and C module files; each file uses a subset of the fields.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct ModuleCase {
  /// The unique name, and the key of the golden capture.
  pub(crate) name: String,
  /// The host that delivers the event.
  pub(crate) host: Host,
  /// The hook entry (`entry` in A, `hook` in C); B always runs `pre-tools`.
  #[serde(default = "pre_tools", alias = "hook")]
  pub(crate) entry: String,
  /// A host-neutral tool event.
  pub(crate) fixture: Option<Value>,
  /// Raw stdin, instead of a rendered fixture.
  pub(crate) stdin: Option<String>,
  /// A Bash command, instead of a fixture.
  pub(crate) command: Option<String>,
  /// Settings files to write.
  pub(crate) settings: Option<BTreeMap<String, String>>,
  /// Start from the shipped settings.
  #[serde(default)]
  pub(crate) shipped_settings: bool,
  /// A config (`{scope, body}` in A, the document or its text elsewhere).
  pub(crate) config: Option<Value>,
  /// Project files to create.
  pub(crate) files: Option<Vec<String>>,
  /// The quality gate status file body.
  pub(crate) gate: Option<String>,
  /// Setup steps.
  pub(crate) setup: Option<Vec<Action>>,
  /// The working directory.
  pub(crate) cwd: Option<PathTag>,
  /// Extra environment.
  pub(crate) env: Option<BTreeMap<String, Value>>,
  /// Run without `python3` on `PATH`.
  #[serde(default)]
  pub(crate) no_python: bool,
  /// Tools to leave off `PATH`.
  pub(crate) without: Option<Vec<String>>,
  /// The decision class.
  pub(crate) expect: String,
  /// Text the decision holds.
  #[serde(default)]
  pub(crate) has: Vec<String>,
  /// Text the decision lacks.
  #[serde(default)]
  pub(crate) lacks: Vec<String>,
  /// Why bash was wrong here; the capture must differ.
  pub(crate) deviation: Option<String>,
}

fn pre_tools() -> String {
  "pre-tools".to_owned()
}

/// What a corpus row expects per host.
#[derive(Debug, Deserialize)]
pub(crate) struct Expect {
  /// On Claude Code.
  pub(crate) claude: String,
  /// On Codex, else as on Claude Code.
  pub(crate) codex: Option<String>,
}

/// The setup of a corpus row, per host.
#[derive(Debug, Deserialize)]
pub(crate) struct HostSetup {
  /// For Claude Code.
  pub(crate) claude: Vec<Action>,
  /// For Codex.
  pub(crate) codex: Vec<Action>,
}

/// A row of the shared `PreToolUse` corpus.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct CorpusCase {
  /// The unique name.
  pub(crate) name: String,
  /// A host-neutral tool event.
  pub(crate) fixture: Option<Value>,
  /// Raw stdin, instead of a rendered fixture.
  pub(crate) stdin: Option<String>,
  /// A project config.
  pub(crate) config: Option<Value>,
  /// Settings lines appended to the shipped settings.
  pub(crate) settings: Option<BTreeMap<String, String>>,
  /// Setup, by host.
  pub(crate) setup_by_host: HostSetup,
  /// The decision class per host.
  pub(crate) expect: Expect,
}

/// What a hook printed and wrote.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub(crate) struct Capture {
  /// Standard output.
  pub(crate) stdout: String,
  /// Standard error.
  pub(crate) stderr: String,
  /// The exit code.
  #[serde(rename = "exitCode")]
  pub(crate) exit_code: i64,
  /// Files the hook changed (C), `null` for a removed one.
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub(crate) files: Option<BTreeMap<String, Option<String>>>,
}

/// A golden file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Golden {
  /// The commit the captures were taken at.
  pub(crate) base: String,
  /// Keyed by case name.
  pub(crate) cases: BTreeMap<String, Capture>,
  /// Keyed by `<corpus row> [<host>]`.
  #[serde(default)]
  pub(crate) corpus: BTreeMap<String, Capture>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CaseFile<T> {
  version: u32,
  cases: Vec<T>,
}

/// The cases of `fixtures/gates/<file>.json`.
pub(crate) fn read_cases<T: DeserializeOwned>(file: &str) -> Res<Vec<T>> {
  let text = fs::read_to_string(format!("{REPO}/fixtures/gates/{file}.json"))?;
  let parsed: CaseFile<T> = serde_json::from_str(&text)?;
  if parsed.version != 1 {
    return Err(format!("{file}: unsupported version {}", parsed.version).into());
  }
  Ok(parsed.cases)
}

/// The golden captures of `fixtures/gates/<file>.json`.
pub(crate) fn read_golden(file: &str) -> Res<Golden> {
  let text = fs::read_to_string(format!("{REPO}/fixtures/gates/{file}.json"))?;
  Ok(serde_json::from_str(&text)?)
}
