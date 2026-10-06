//! The shared config fixture (AC-1, AC-3): the Rust loader and resolvers
//! reproduce every case of `fixtures/config/expected.json`, as the TypeScript
//! loader does (`packages/toolu-core/src/config/__tests__/config-fixture.test.ts`).
//! Values compare as `JSON.stringify` text, so `1` and `1.0` agree.

#[path = "helpers/repo.rs"]
mod repo;

use std::path::{Path, PathBuf};

use repo::Res;
use serde_json::{Value, json};
use toolu_protocol::host::Host;
use toolu_runtime::config::docs_sync::{DocsSyncKey, docs_sync_globs};
use toolu_runtime::config::gate_mode::{gate_mode, gate_preset};
use toolu_runtime::config::load::{LoadedConfig, load};
use toolu_runtime::config::quality::{
  QualityLang, ThresholdSource, quality_flag, quality_threshold, ts_max_file_lines_resolved,
};
use toolu_runtime::config::read::{
  codex_model, enabled, enabled_explicit, flag_false, flag_true, model,
};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::json::stringify;

const FIXTURE: &str = "fixtures/config/expected.json";

fn host(name: &str) -> Res<Host> {
  Host::parse(name).ok_or_else(|| format!("unknown host {name}"))
}

fn lang(name: &str) -> Res<QualityLang> {
  [QualityLang::Ts, QualityLang::Rust, QualityLang::Python]
    .into_iter()
    .find(|lang| lang.name() == name)
    .ok_or_else(|| format!("unknown lang {name}"))
}

fn docs_key(name: &str) -> Res<DocsSyncKey> {
  [
    DocsSyncKey::Surfaces,
    DocsSyncKey::SurfaceExcludes,
    DocsSyncKey::CodeSurfaces,
  ]
  .into_iter()
  .find(|key| key.name() == name)
  .ok_or_else(|| format!("unknown docsSync key {name}"))
}

fn source(source: ThresholdSource) -> &'static str {
  match source {
    ThresholdSource::Override => "override",
    ThresholdSource::Native => "native",
    ThresholdSource::Default => "default",
  }
}

/// The value of the call `id` names, e.g. `gateMode pushReview codex`.
fn call(config: &LoadedConfig, id: &str, root: &Path) -> Res<Value> {
  let words: Vec<&str> = id.split(' ').collect();
  let (a, b) = (
    words.get(1).copied().unwrap_or(""),
    words.get(2).copied().unwrap_or(""),
  );
  Ok(match words.first().copied().unwrap_or("") {
    "gatePreset" => json!(gate_preset(config)),
    "gateMode" => json!(gate_mode(config, a, Some(host(b)?), None).name()),
    "model" => json!(model(config, a).map_err(|err| err.0)?),
    "codexModel" => {
      let pair = codex_model(config, a).map_err(|err| err.0)?;
      json!({ "model": pair.model, "reasoningEffort": pair.reasoning_effort })
    }
    "qualityThreshold" => json!(quality_threshold(config, lang(a)?, b, Some(root))),
    "tsMaxFileLinesResolved" => {
      let (value, from) = ts_max_file_lines_resolved(config, Some(root));
      json!({ "value": value, "source": source(from) })
    }
    "qualityFlag" => json!(quality_flag(config, lang(a)?, b, true)),
    "docsSync" => json!(docs_sync_globs(config, docs_key(a)?)),
    "enabled" => json!(enabled(config, a, b)),
    "flagTrue" => json!(flag_true(config, a, b)),
    "flagFalse" => json!(flag_false(config, a, b)),
    "enabledExplicit" => json!(enabled_explicit(config, a, b)),
    other => return Err(format!("unknown call {other}")),
  })
}

fn placement(value: Option<&Value>) -> Res<Option<String>> {
  let Some(value) = value.filter(|value| !value.is_null()) else {
    return Ok(None);
  };
  if let Some(text) = value.get("text").and_then(Value::as_str) {
    return Ok(Some(text.to_owned()));
  }
  let file = value
    .get("file")
    .and_then(Value::as_str)
    .ok_or("a placement names a file or a text")?;
  std::fs::read_to_string(repo::path("fixtures/config").join(file))
    .map(Some)
    .map_err(|err| err.to_string())
}

fn write(path: &Path, text: Option<String>) -> Res<()> {
  let Some(text) = text else {
    return Ok(());
  };
  std::fs::create_dir_all(path.parent().ok_or("no parent")?).map_err(|err| err.to_string())?;
  std::fs::write(path, text).map_err(|err| err.to_string())
}

fn same(label: &str, got: &Value, want: &Value) -> Res<()> {
  if stringify(got) == stringify(want) {
    Ok(())
  } else {
    Err(format!(
      "{label}: got {}, expected {}",
      stringify(got),
      stringify(want)
    ))
  }
}

fn run(case: &Value) -> Res<usize> {
  let host = host(case.get("host").and_then(Value::as_str).unwrap_or(""))?;
  let dir = tempfile::tempdir().map_err(|err| err.to_string())?;
  let root = std::fs::canonicalize(dir.path()).map_err(|err| err.to_string())?;
  let (home, project) = (root.join("home"), root.join("project"));
  std::fs::create_dir_all(&project).map_err(|err| err.to_string())?;
  let dirname = format!(".{}", host.name());
  let user_file = home.join(&dirname).join("toolu.config.json");
  let project_file: PathBuf = project.join(&dirname).join("toolu.config.json");
  write(&user_file, placement(case.get("user"))?)?;
  write(&project_file, placement(case.get("project"))?)?;
  let tokens = |text: &str| {
    let text = text.replace(&user_file.display().to_string(), "$USER_CONFIG");
    text.replace(&project_file.display().to_string(), "$PROJECT_CONFIG")
  };
  let env = Env::from_pairs([
    ("HOME", home.display().to_string()),
    ("TOOLU_PROJECT_DIR", project.display().to_string()),
  ]);
  let config = load(&Roots::new(env, Some(host)), None);
  let expect = case.get("expect").ok_or("no expect")?;
  same(
    "invalid",
    &json!(config.invalid.as_deref().map(tokens)),
    &expect["invalid"],
  )?;
  let warnings: Vec<String> = config
    .take_warnings()
    .iter()
    .map(|text| tokens(text))
    .collect();
  same("warnings", &json!(warnings), &expect["warnings"])?;
  same("data", &Value::Object(config.data.clone()), &expect["data"])?;
  let resolved = expect
    .get("resolved")
    .and_then(Value::as_object)
    .ok_or("no resolved")?;
  for (id, want) in resolved {
    same(id, &call(&config, id, &project)?, &want["value"])?;
    let want_warnings = want.get("warnings").cloned().unwrap_or_else(|| json!([]));
    same(
      &format!("{id} warnings"),
      &json!(config.take_warnings()),
      &want_warnings,
    )?;
  }
  Ok(resolved.len())
}

#[test]
fn every_config_case_resolves_as_typescript_does() {
  let cases = repo::cases(FIXTURE).unwrap();
  assert_eq!(cases.len(), 35);
  let mut failures = Vec::new();
  let mut calls = 0;
  for case in &cases {
    let name = case.get("name").and_then(Value::as_str).unwrap_or("?");
    match run(case) {
      Ok(count) => calls += count,
      Err(err) => failures.push(format!("{name}: {err}")),
    }
  }
  assert!(failures.is_empty(), "{failures:#?}");
  assert_eq!(calls, 35 * 81);
}
