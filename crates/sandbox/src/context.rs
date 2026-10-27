//! Everything a verb reads before it decides: the environment (with
//! `--config-dir` applied), the machine's facts, the `sandbox` settings and the
//! project this directory belongs to. An invalid config fails closed.

use std::path::Path;

use serde_json::{Map, Value};
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::Ctx;
use toolu_runtime::config::load::{LoadedConfig, check_text, config_files, load};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::facts::Facts;
use crate::project::{Project, detect};
use crate::settings::{Resolved, parse, resolve, user_only_keys};
use crate::step::Failure;

/// What a verb works from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Context {
  /// The environment, `TOOLU_CONFIG_DIR` set from `--config-dir`.
  pub(crate) env: Env,
  /// This machine's facts.
  pub(crate) facts: Facts,
  /// The settings, defaults filled in.
  pub(crate) settings: Resolved,
  /// The project of the current directory.
  pub(crate) project: Project,
}

impl Context {
  /// The context of this process.
  ///
  /// # Errors
  /// When the current directory cannot be read or the config is invalid.
  pub(crate) fn resolve(ctx: &Ctx) -> Result<Context, Failure> {
    let cwd = std::env::current_dir().map_err(|err| {
      Failure::new(
        Exit::Failure,
        format!("cannot read the current directory: {err}"),
      )
    })?;
    Context::at(ctx, env_of(ctx), &cwd, Facts::probe())
  }

  /// The context for `cwd` with `env` and `facts`.
  ///
  /// # Errors
  /// When the toolu config or its `sandbox` section is invalid.
  pub(crate) fn at(ctx: &Ctx, env: Env, cwd: &Path, facts: Facts) -> Result<Context, Failure> {
    let roots = Roots::new(env.clone(), ctx.host);
    let config = load(&roots, Some(cwd));
    if let Some(why) = &config.invalid {
      return Err(Failure::new(
        Exit::Failure,
        format!("the toolu config is invalid: {why}"),
      ));
    }
    let section = user_only_keys(sandbox_section(&config)?, &user_section(&roots, cwd));
    let written =
      parse(section.as_ref(), &env.home()).map_err(|why| Failure::new(Exit::Failure, why))?;
    Ok(Context {
      settings: resolve(written, &facts),
      project: detect(&env, cwd),
      facts,
      env,
    })
  }
}

/// The `sandbox` section, unset when absent or `null`.
///
/// # Errors
/// Exit 1 when it is anything but an object, which would read as the defaults.
fn sandbox_section(config: &LoadedConfig) -> Result<Option<&Map<String, Value>>, Failure> {
  match config.data.get("sandbox") {
    None | Some(Value::Null) => Ok(None),
    Some(Value::Object(section)) => Ok(Some(section)),
    Some(other) => Err(Failure::new(
      Exit::Failure,
      format!("sandbox must be an object, not {other}"),
    )),
  }
}

/// The `sandbox` section of the user's own config file alone, empty when absent.
fn user_section(roots: &Roots, cwd: &Path) -> Map<String, Value> {
  let text = std::fs::read_to_string(config_files(roots, Some(cwd)).user).ok();
  let section = text
    .and_then(|text| check_text(&text).ok())
    .and_then(|mut data| data.remove("sandbox"));
  match section {
    Some(Value::Object(section)) => section,
    _ => Map::new(),
  }
}

/// The process environment, with `--config-dir` as `TOOLU_CONFIG_DIR`.
fn env_of(ctx: &Ctx) -> Env {
  let env = Env::process();
  match &ctx.config_dir {
    Some(dir) => env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy()),
    None => env,
  }
}

#[cfg(test)]
#[path = "tests/context_test.rs"]
mod tests;
