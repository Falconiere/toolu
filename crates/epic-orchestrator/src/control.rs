//! Foreground engine, ensure, start and the service unit.

use std::net::{IpAddr, SocketAddr};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use clap::ArgMatches;
use serde_json::{Value, json};
use toolu_engine::babysit::BabysitTick;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Outcome;
use toolu_runtime::config::{epic, load, secrets};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;
use toolu_runtime::invocation::current_exe;
use toolu_runtime::process::{Spec, detach};

use crate::PROTOCOL;
use crate::client::exchange_retry;
use crate::disk::{read_value, write_value};
use crate::http::Config as HttpConfig;
use crate::lock::live;
use crate::notify::AttentionOptions;
use crate::paths::Paths;
use crate::server::Fault;
use crate::socket::{ServeOptions, serve_with_http};
use crate::verbs::{failed, text};

/// Run, ensure or replace the resident engine.
pub(crate) fn engine(matches: &ArgMatches, env: &Env, tick: &dyn BabysitTick) -> Outcome {
  if matches.get_flag("replace") && !replace_engine(&Paths::from_env(env), REPLACE_WAIT) {
    return Outcome::data(DRAINING.to_owned());
  }
  if matches.get_flag("ensure") {
    return match ensure(env) {
      Ok(()) => Outcome::data(String::new()),
      Err(err) => failed("toolu epic engine", &err),
    };
  }
  foreground_with_http(env, matches, tick)
}

/// Run the explicit `epic serve` entry.
pub(crate) fn serve(matches: &ArgMatches, env: &Env, tick: &dyn BabysitTick) -> Outcome {
  foreground_with_http(env, matches, tick)
}

fn foreground_with_http(env: &Env, matches: &ArgMatches, tick: &dyn BabysitTick) -> Outcome {
  let paths = Paths::from_env(env);
  let scripts = env.get("TOOLU_EPIC_SCRIPTS").map(PathBuf::from);
  let (http, attention) = match http_config(env, matches)
    .and_then(|http| attention_options(env, http.as_ref()).map(|attention| (http, attention)))
  {
    Ok(options) => options,
    Err(err) => return failed("toolu epic engine", &err),
  };
  match serve_with_http(
    paths,
    scripts,
    fault_of(env),
    PROTOCOL,
    ServeOptions {
      http,
      tick,
      attention,
    },
  ) {
    Ok(()) => Outcome::data(String::new()),
    Err(err) if err == "fault" => {
      Outcome::failed(Exit::TempFail, "toolu epic engine: fault".to_owned())
    }
    Err(err) => failed("toolu epic engine", &err),
  }
}

fn http_config(env: &Env, matches: &ArgMatches) -> Result<Option<HttpConfig>, String> {
  if matches.get_flag("no-http") {
    return Ok(None);
  }
  let roots = Roots::new(env.clone(), None);
  let settings = epic::parse(&load::load(&roots, None)).map_err(|err| err.to_string())?;
  let addr: SocketAddr = match matches.get_one::<String>("http") {
    Some(addr) => addr
      .parse()
      .map_err(|_err| "invalid --http address".to_owned())?,
    None => SocketAddr::new(
      settings
        .http_bind
        .parse::<IpAddr>()
        .map_err(|_err| "invalid epic.http bind address".to_owned())?,
      settings.http_port,
    ),
  };
  let secrets = secrets::load(&roots).map_err(|err| err.to_string())?;
  Ok(Some(HttpConfig {
    reload: Some(roots),
    herdr: crate::http_live::Herdr::of(env),
    attach_session: crate::herdr::session_name(env).map(str::to_owned),
    ..HttpConfig::new(addr, secrets, settings.peers)
  }))
}

/// Reminder delay and notifier settings from `epic.attention`.
pub(crate) fn attention_options(
  env: &Env,
  http: Option<&HttpConfig>,
) -> Result<AttentionOptions, String> {
  let roots = Roots::new(env.clone(), None);
  let settings = epic::parse(&load::load(&roots, None)).map_err(|err| err.to_string())?;
  let herdr = settings.attention_herdr && crate::source::prompt_enabled();
  let notify = (herdr || settings.attention_enabled).then(|| crate::notify::Settings {
    dashboard: http.map(|config| crate::notify_notice::dashboard(config.addr)),
    ..crate::notify::Settings::new(env.clone(), herdr, settings.attention_enabled)
  });
  Ok(AttentionOptions {
    remind_ms: settings.remind_after_minutes.saturating_mul(60_000),
    notify,
  })
}

/// Interpret the scripted test fault selector.
pub(crate) fn fault_of(env: &Env) -> Fault {
  match env.get("TOOLU_EPIC_FAULT") {
    Some("before-merge") => Fault::BeforeMerge,
    Some("after-merge") => Fault::AfterMerge,
    Some("before-merge-record") => Fault::BeforeMergeRecord,
    _ => Fault::None,
  }
}

/// Start the engine when a registry exists and no owner is live. A
/// malformed `registry.json` is an error, not an empty registry.
pub(crate) fn ensure(env: &Env) -> Result<(), String> {
  let paths = Paths::from_env(env);
  let registry = read_value(&paths.registry())?;
  let registered = registry
    .get("epics")
    .and_then(Value::as_array)
    .is_some_and(|rows| !rows.is_empty());
  if !registered || live(&paths.lock()) {
    return Ok(());
  }
  detach_engine(env)
}

/// Detach a new resident engine process.
pub(crate) fn detach_engine(env: &Env) -> Result<(), String> {
  let exe = current_exe().ok_or_else(|| "cannot find the current executable".to_owned())?;
  let mut spec = Spec::new([
    exe.display().to_string(),
    "epic".to_owned(),
    "engine".to_owned(),
  ]);
  spec.env = Some(env.clone());
  detach(&spec).map(|_| ()).map_err(|err| format!("{err:?}"))
}

/// How long `engine --replace` waits for the old engine's running effect.
const REPLACE_WAIT: Duration = Duration::from_secs(60);

/// What `engine --replace` prints when the old engine still drains.
pub(crate) const DRAINING: &str = "engine draining; the new engine starts when it exits";

/// Ask a live engine to drain and wait up to `wait` for its lock to free;
/// `false` while it still runs an effect (the next `ensure` starts the new one).
pub(crate) fn replace_engine(paths: &Paths, wait: Duration) -> bool {
  if paths.socket().exists() {
    let _replaced = exchange_retry(paths, PROTOCOL, &json!({"op": "replace"}));
  }
  let deadline = std::time::Instant::now() + wait;
  while live(&paths.lock()) {
    if std::time::Instant::now() >= deadline {
      return false;
    }
    thread::sleep(Duration::from_millis(20));
  }
  true
}

/// Register one epic and launch the engine if needed.
pub(crate) fn start(matches: &ArgMatches, env: &Env) -> Outcome {
  let Some(dir) = text(matches, "state-dir") else {
    return failed("toolu epic start", "state-dir is required");
  };
  if !Path::new(dir).is_dir() {
    return failed(
      "toolu epic start",
      &format!("no epic state directory {dir}"),
    );
  }
  let paths = Paths::from_env(env);
  let settings = crate::epics::LaunchSettings {
    safe: matches.get_flag("safe"),
    no_jev: matches.get_flag("no-jev"),
  };
  // The settings first: a registered epic never runs without them.
  let saved = crate::epics::write_launch_settings(Path::new(dir), settings)
    .and_then(|()| register(&paths, dir));
  if let Err(err) = saved {
    return failed("toolu epic start", &err);
  }
  if live(&paths.lock()) {
    return Outcome::data(String::new());
  }
  match detach_engine(env) {
    Ok(()) => Outcome::data(String::new()),
    Err(err) => failed("toolu epic start", &err),
  }
}

/// Add an epic directory to the registry once.
pub(crate) fn register(paths: &Paths, state_dir: &str) -> Result<(), String> {
  let current = read_value(&paths.registry())?;
  let mut epics = current
    .get("epics")
    .and_then(Value::as_array)
    .cloned()
    .unwrap_or_default();
  let present = epics
    .iter()
    .any(|row| row.get("state_dir").and_then(Value::as_str) == Some(state_dir));
  if !present {
    epics.push(json!({"key": epic_key(Path::new(state_dir)), "state_dir": state_dir}));
  }
  write_value(&paths.registry(), &json!({"version": 1, "epics": epics}))
}

/// Resolve the registered key from the graph or directory name.
pub(crate) fn epic_key(state_dir: &Path) -> String {
  if let Ok(graph) = read_value(&state_dir.join("graph.json"))
    && let Some(key) = graph
      .get("epic")
      .and_then(Value::as_str)
      .filter(|key| !key.is_empty())
  {
    return key.to_owned();
  }
  state_dir
    .file_name()
    .and_then(|name| name.to_str())
    .map_or_else(|| state_dir.display().to_string(), str::to_owned)
}

/// Whether at least one epic is registered.
pub(crate) fn registry_nonempty(paths: &Paths) -> bool {
  let Ok(value) = read_value(&paths.registry()) else {
    return false;
  };
  value
    .get("epics")
    .and_then(Value::as_array)
    .is_some_and(|rows| !rows.is_empty())
}

#[cfg(test)]
#[path = "tests/control_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/control_replace_test.rs"]
mod replace_tests;
