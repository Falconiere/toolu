//! The shared host-roots fixture (AC-2): `Roots` reproduces every case of
//! `fixtures/host/root.json`, as `host-roots.ts` does
//! (`packages/toolu-core/src/host/__tests__/host-roots.test.ts`), in the same
//! kind of sandbox: a temp root holding `project` (a real git repository when
//! the case asks for one) and `home`.

#[path = "helpers/repo.rs"]
mod repo;

use std::path::{Path, PathBuf};
use std::process::Command;

use repo::Res;
use serde_json::{Map, Value};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::host::snapshot::codex_plugin_snapshot_path;

const FIXTURE: &str = "fixtures/host/root.json";

struct Sandbox {
  _dir: tempfile::TempDir,
  root: PathBuf,
}

impl Sandbox {
  fn new(git: bool) -> Res<Sandbox> {
    let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
    let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
    for sub in ["project", "home"] {
      std::fs::create_dir_all(root.join(sub)).map_err(|err| err.to_string())?;
    }
    if git {
      let status = Command::new("git")
        .args(["init", "-q"])
        .current_dir(root.join("project"))
        .status();
      status
        .map_err(|err| err.to_string())?
        .success()
        .then_some(())
        .ok_or("git init failed")?;
    }
    Ok(Sandbox { _dir: dir, root })
  }

  /// A `$PROJECT`, `$ROOT` or `$HOME` path, or a project-relative one.
  fn path(&self, text: &str) -> PathBuf {
    let tokens = [("$PROJECT", "project"), ("$HOME", "home"), ("$ROOT", "")];
    for (token, sub) in tokens {
      if let Some(rest) = text.strip_prefix(token) {
        return PathBuf::from(
          format!("{}{rest}", self.root.join(sub).display()).replace("//", "/"),
        );
      }
    }
    self.root.join("project").join(text)
  }

  /// `value` with `$path` and `$runtime` objects resolved, as the TypeScript runner does.
  fn materialize(&self, value: &Value) -> Value {
    match value {
      Value::Array(items) => items.iter().map(|item| self.materialize(item)).collect(),
      Value::Object(map) => match single(map) {
        Some(("$path", Value::String(text))) => Value::from(self.path(text).display().to_string()),
        Some(("$runtime", Value::String(name))) => Value::from(runtime(name)),
        _ => map
          .iter()
          .map(|(key, item)| (key.clone(), self.materialize(item)))
          .collect(),
      },
      Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => value.clone(),
    }
  }
}

fn single(map: &Map<String, Value>) -> Option<(&str, &Value)> {
  let mut entries = map.iter();
  let first = entries.next()?;
  entries
    .next()
    .is_none()
    .then_some((first.0.as_str(), first.1))
}

fn runtime(name: &str) -> String {
  match name {
    "PATH" => std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".to_owned()),
    _ => std::env::home_dir()
      .unwrap_or_default()
      .join(".claude")
      .display()
      .to_string(),
  }
}

/// The `Roots` and the `cwd`/`root` options of one options argument.
fn options(value: &Value) -> Res<(Roots, Option<PathBuf>, Option<PathBuf>)> {
  let env = value
    .get("env")
    .and_then(Value::as_object)
    .cloned()
    .unwrap_or_default();
  let pairs = env
    .iter()
    .map(|(key, item)| (key.clone(), item.as_str().unwrap_or_default().to_owned()));
  let host = match value.get("host").and_then(Value::as_str) {
    Some(name) => Some(Host::parse(name).ok_or(format!("unknown host {name}"))?),
    None => None,
  };
  let path = |key: &str| value.get(key).and_then(Value::as_str).map(PathBuf::from);
  Ok((
    Roots::new(Env::from_pairs(pairs), host),
    path("cwd"),
    path("root"),
  ))
}

fn text(value: Option<&Value>) -> Res<&str> {
  value
    .and_then(Value::as_str)
    .ok_or_else(|| "a string argument".to_owned())
}

fn shown(path: Option<PathBuf>) -> Value {
  path.map_or(Value::Null, |path| Value::from(path.display().to_string()))
}

/// Calls `function` on `args`; `Err(())` is a caller error.
fn call(function: &str, args: &[Value]) -> Res<Result<Value, ()>> {
  let options_at = |index: usize| options(args.get(index).unwrap_or(&Value::Null));
  let (roots, cwd, root) = options_at(match function {
    "projectStateDir" | "pluginInstallCommand" => 1,
    "invocation" => 2,
    _ => 0,
  })?;
  let (cwd, root) = (cwd.as_deref(), root.as_deref());
  let value = match function {
    "configRoot" => shown(Some(roots.config_root())),
    "projectRoot" => shown(roots.project_root(cwd)),
    "projectConfigPath" => shown(roots.project_config_path(cwd)),
    "projectStateRoot" => shown(roots.project_state_root(cwd, root)),
    "projectDirname" => Value::from(roots.project_dirname()),
    "pluginRoot" => shown(roots.plugin_root()),
    "pluginData" => shown(roots.plugin_data()),
    "projectStateDir" => match roots.project_state_dir(text(args.first())?, cwd, root) {
      Ok(path) => shown(path),
      Err(_) => return Ok(Err(())),
    },
    "invocation" => match roots.invocation(text(args.first())?, text(args.get(1))?) {
      Ok(line) => Value::from(line),
      Err(_) => return Ok(Err(())),
    },
    "pluginInstallCommand" => match roots.plugin_install_command(text(args.first())?) {
      Ok(command) => command.map_or(Value::Null, Value::from),
      Err(_) => return Ok(Err(())),
    },
    other => return Err(format!("unknown function {other}")),
  };
  Ok(Ok(value))
}

fn run_calls(case: &Value) -> Res<usize> {
  let sandbox = Sandbox::new(case.get("git") == Some(&Value::Bool(true)))?;
  for dir in case
    .get("dirs")
    .and_then(Value::as_array)
    .into_iter()
    .flatten()
  {
    let dir = sandbox.path(text(Some(dir))?);
    std::fs::create_dir_all(dir).map_err(|err| err.to_string())?;
  }
  let checks = case
    .get("checks")
    .and_then(Value::as_array)
    .ok_or("no checks")?;
  for check in checks {
    let function = text(check.get("fn"))?;
    let args = sandbox.materialize(check.get("args").unwrap_or(&Value::Null));
    let got = call(function, args.as_array().map_or(&[][..], Vec::as_slice))?;
    let expected = match check.get("throws") {
      Some(_) => Err(()),
      None => Ok(sandbox.materialize(check.get("expected").unwrap_or(&Value::Null))),
    };
    if got != expected {
      return Err(format!(
        "{function} {args}: got {got:?}, expected {expected:?}"
      ));
    }
  }
  Ok(checks.len())
}

/// Two resolutions of a detected host each carry the warning exactly once.
fn run_warning(case: &Value) -> Res<usize> {
  let sandbox = Sandbox::new(false)?;
  let home = sandbox.path("$HOME").display().to_string();
  let pairs = [
    ("PATH", runtime("PATH")),
    ("HOME", home),
    (
      "TOOLU_HOST_OVERRIDE",
      text(case.get("override"))?.to_owned(),
    ),
    (
      "TOOLU_PROJECT_DIR",
      text(case.get("projectDir"))?.to_owned(),
    ),
  ];
  let warning = text(case.get("warning"))?;
  let config = Roots::new(Env::from_pairs(pairs.clone()), None);
  let snapshot = Roots::new(Env::from_pairs(pairs), None);
  let project = config
    .project_config_path(None)
    .ok_or("no project config path")?;
  if !project.starts_with(Path::new(text(case.get("projectDir"))?)) {
    return Err(format!("project config path {}", project.display()));
  }
  let _path = codex_plugin_snapshot_path(&snapshot);
  for roots in [&config, &snapshot] {
    if roots.warning() != Some(warning) {
      return Err(format!("warning {:?}", roots.warning()));
    }
  }
  Ok(2)
}

#[test]
fn every_host_root_case_resolves_as_typescript_does() {
  let cases = repo::cases(FIXTURE).unwrap();
  assert_eq!(cases.len(), 18);
  let mut failures = Vec::new();
  let mut checks = 0;
  for case in &cases {
    let name = case.get("name").and_then(Value::as_str).unwrap_or("?");
    let outcome = match case.get("kind").and_then(Value::as_str) {
      Some("calls") => run_calls(case),
      Some("warning") => run_warning(case),
      other => Err(format!("unknown kind {other:?}")),
    };
    match outcome {
      Ok(count) => checks += count,
      Err(err) => failures.push(format!("{name}: {err}")),
    }
  }
  assert!(failures.is_empty(), "{failures:#?}");
  assert!(checks > 40, "only {checks} checks ran");
}
