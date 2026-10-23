//! The epic engine's systemd user unit.

use std::path::Path;

use toolu_runtime::atomic::write_atomic;
use toolu_runtime::cli::Outcome;
use toolu_runtime::env::Env;
use toolu_runtime::invocation::current_exe;

use crate::paths::Paths;
use crate::verbs::failed;

/// Install the engine's user service unit.
pub(crate) fn service(env: &Env) -> Outcome {
  let paths = Paths::from_env(env);
  let Some(exe) = current_exe() else {
    return failed("toolu epic service", "cannot find the current executable");
  };
  match install_unit(&paths, &env.home(), &exe, env) {
    Ok(()) => Outcome::data(format!("toolu epic: unit {}", paths.service().display())),
    Err(err) => failed("toolu epic service", &err),
  }
}

/// Write the service unit to the resource and user configuration roots.
pub(crate) fn install_unit(
  paths: &Paths,
  home: &Path,
  exe: &Path,
  env: &Env,
) -> Result<(), String> {
  let executable = unit_word(&exe.display().to_string())?;
  let resource = unit_word(&format!("TOOLU_RESOURCE_HOME={}", paths.root.display()))?;
  let mut body = format!(
    "[Unit]\nDescription=toolu epic engine\n\n[Service]\nExecStart={executable} epic engine\nEnvironment={resource}\n"
  );
  for key in [
    "PATH",
    "HERDR_ENV",
    "HERDR_SOCKET_PATH",
    "HERDR_BIN_PATH",
    "HERDR_CONFIG_PATH",
    "HERDR_SESSION",
    "TOOLU_EPIC_HERDR_SESSION",
  ] {
    if let Some(value) = env.get(key) {
      let assignment = unit_word(&format!("{key}={value}"))?;
      body.push_str("Environment=");
      body.push_str(&assignment);
      body.push('\n');
    }
  }
  body.push_str(
    "EnvironmentFile=-%h/.config/toolu/epic.env\nRestart=on-failure\nRestartSec=5s\n\n[Install]\nWantedBy=default.target\n",
  );
  let user = home.join(".config/systemd/user/toolu-epic.service");
  if !write_atomic(&paths.service(), &body) {
    return Err(format!("could not write {}", paths.service().display()));
  }
  if !write_atomic(&user, &body) {
    return Err(format!("could not write {}", user.display()));
  }
  Ok(())
}

fn unit_word(value: &str) -> Result<String, String> {
  if value.chars().any(|ch| matches!(ch, '\n' | '\r' | '\0')) {
    return Err("systemd unit value contains a line break or NUL".to_owned());
  }
  let mut quoted = String::with_capacity(value.len() + 2);
  quoted.push('"');
  for ch in value.chars() {
    match ch {
      '"' => quoted.push_str("\\\""),
      '\\' => quoted.push_str("\\\\"),
      '%' => quoted.push_str("%%"),
      _ => quoted.push(ch),
    }
  }
  quoted.push('"');
  Ok(quoted)
}

#[cfg(test)]
#[path = "tests/service_test.rs"]
mod tests;
