//! Native herdr-core CLI transport. Addresses are opaque, including `machine/t7`.

/// Remote committed-snapshot and file transport.
pub mod remote;

use std::time::Duration;

use serde_json::Value;
use toolu_runtime::env::Env;
use toolu_runtime::process::{Output, RunError, Spec, run};

/// A structured hc failure. Uncertain mutations must be inspected before retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
  /// Upstream error kind, or a transport failure.
  pub code: String,
  /// Diagnostic without credentials or command input.
  pub message: String,
  /// The request may already have taken effect.
  pub uncertain: bool,
}

impl Error {
  /// A locally detected failure.
  pub fn new(code: &str, message: impl Into<String>) -> Self {
    Self {
      code: code.into(),
      message: message.into(),
      uncertain: false,
    }
  }
}

/// A client with an explicit environment, suitable for isolated `HC_SESSION`s.
pub struct Client<'a> {
  env: &'a Env,
}

impl<'a> Client<'a> {
  /// Use the caller's captured environment; never load a project env file.
  #[must_use]
  pub fn new(env: &'a Env) -> Self {
    Self { env }
  }

  /// Run a JSON command with its own deadline.
  ///
  /// # Errors
  /// hc returns a structured error, invalid output, or cannot be reached.
  pub fn call(&self, args: &[&str], timeout: Duration) -> Result<Value, Error> {
    validate_target(args)?;
    let mut spec = Spec::new(["hc", "--json"]);
    spec.argv.extend(args.iter().map(|arg| (*arg).to_owned()));
    spec.env = Some(self.env.clone());
    spec.timeout = timeout;
    let output = run(&spec).map_err(|error| process_error(&error))?;
    decode(&output)
  }

  /// One terminal's identity and reported status.
  ///
  /// # Errors
  /// The address is absent or hc is unavailable.
  pub fn get(&self, address: &str) -> Result<Value, Error> {
    self.call(&["get", address], Duration::from_secs(30))
  }

  /// Submit once. Never force a blocked dialog or automatically resend.
  ///
  /// # Errors
  /// hc refuses the input, or its delivery is uncertain.
  pub fn send(&self, address: &str, text: &str) -> Result<Value, Error> {
    self.call(&["send", address, "--", text], Duration::from_secs(60))
  }

  /// Start a controller-independent terminal without changing desktop focus.
  /// Persist `name` and ownership before calling, then persist the returned address.
  ///
  /// # Errors
  /// Launch fails or returns no terminal identity.
  pub fn launch(&self, launch: &Launch<'_>) -> Result<Value, Error> {
    validate_launch(launch)?;
    let mut args = vec![
      "run",
      "--bg",
      "--no-parent",
      "--no-notify",
      "--outlive-parent",
      "--name",
      launch.name,
      "--cwd",
      launch.cwd,
      "--label",
      launch.owner,
    ];
    if let Some(machine) = launch.machine {
      args.extend(["--on", machine]);
    }
    args.push("--");
    args.extend(launch.argv.iter().map(String::as_str));
    let terminal = self.call(&args, Duration::from_secs(120))?;
    let raw = address(&terminal).map_err(uncertain)?;
    let target_address = qualified(raw, launch.machine).map_err(uncertain)?;
    let mut verified = self.get(&target_address).map_err(uncertain)?;
    if qualified(address(&verified).map_err(uncertain)?, launch.machine).map_err(uncertain)?
      != target_address
    {
      return Err(uncertain(Error::new(
        "identity_mismatch",
        "launched terminal address differs",
      )));
    }
    require_owner(&verified, launch.owner).map_err(uncertain)?;
    *verified
      .get_mut("address")
      .ok_or_else(|| uncertain(Error::new("invalid_output", "missing address")))? =
      Value::String(target_address);
    Ok(verified)
  }

  /// End only a terminal bearing the recorded ownership label.
  ///
  /// # Errors
  /// Ownership differs, or shutdown cannot be verified.
  pub fn kill_owned(&self, address: &str, owner: &str) -> Result<(), Error> {
    let terminal = self.get(address)?;
    require_owner(&terminal, owner)?;
    self.call(&["kill", address], Duration::from_secs(30))?;
    if self.get(address)?.get("state").and_then(Value::as_str) != Some("finished") {
      return Err(Error::new("cleanup_incomplete", "terminal is still live"));
    }
    Ok(())
  }
}

/// Inputs to one native launch; argv includes the host binary.
pub struct Launch<'a> {
  /// Durable, unique hc name, saved before launch for lost-reply recovery.
  pub name: &'a str,
  /// Machine-side working directory.
  pub cwd: &'a str,
  /// `toolu.owner=<attempt identity>`.
  pub owner: &'a str,
  /// Configured machine, or this one.
  pub machine: Option<&'a str>,
  /// Arguments passed directly, without a shell.
  pub argv: &'a [String],
}

/// The immutable terminal address.
///
/// # Errors
/// Upstream omitted an identity.
pub fn address(terminal: &Value) -> Result<&str, Error> {
  terminal
    .get("address")
    .and_then(Value::as_str)
    .filter(|s| !s.is_empty())
    .ok_or_else(|| Error::new("invalid_output", "hc omitted terminal address"))
}

/// Use the program's own status, never screen heuristics.
#[must_use]
pub fn status(terminal: &Value) -> &str {
  if terminal.get("state").and_then(Value::as_str) == Some("finished") {
    return "done";
  }
  match terminal.pointer("/status/state").and_then(Value::as_str) {
    Some(word @ ("idle" | "working" | "blocked" | "done" | "error")) => word,
    _ => "unknown",
  }
}

/// Check ownership before a destructive operation or recovered adoption.
///
/// # Errors
/// The saved attempt does not own this terminal.
pub fn require_owner(terminal: &Value, owner: &str) -> Result<(), Error> {
  let expected = owner.strip_prefix("toolu.owner=").unwrap_or(owner);
  if expected.is_empty()
    || terminal
      .pointer("/labels/toolu.owner")
      .and_then(Value::as_str)
      != Some(expected)
  {
    return Err(Error::new(
      "identity_mismatch",
      "terminal belongs to another attempt",
    ));
  }
  Ok(())
}

fn uncertain(mut error: Error) -> Error {
  error.uncertain = true;
  error
}

fn process_error(error: &RunError) -> Error {
  let may_run = matches!(
    error,
    RunError::Wait(_) | RunError::Stdin(_) | RunError::Callback(_)
  );
  Error {
    code: "transport".into(),
    message: format!("{error:?}"),
    uncertain: may_run,
  }
}

fn valid_target(value: &str) -> bool {
  !value.is_empty() && !value.starts_with('-') && !value.chars().any(char::is_control)
}

fn validate_target(args: &[&str]) -> Result<(), Error> {
  if matches!(
    args.first().copied(),
    Some("get" | "send" | "kill" | "read" | "wait" | "interrupt")
  ) && args.get(1).is_none_or(|value| !valid_target(value))
  {
    return Err(Error::new("invalid_target", "invalid terminal target"));
  }
  Ok(())
}

fn validate_launch(launch: &Launch<'_>) -> Result<(), Error> {
  let name = launch.name;
  let owner = launch.owner.strip_prefix("toolu.owner=").unwrap_or("");
  if name.is_empty()
    || name.len() > 32
    || !name
      .bytes()
      .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
    || owner.is_empty()
    || owner.chars().any(char::is_control)
    || !std::path::Path::new(launch.cwd).is_absolute()
    || launch.cwd.chars().any(char::is_control)
    || launch
      .machine
      .is_some_and(|m| !valid_target(m) || m.contains('/'))
    || launch.argv.first().is_none_or(String::is_empty)
  {
    return Err(Error::new(
      "invalid_launch",
      "invalid terminal launch identity or command",
    ));
  }
  Ok(())
}

fn qualified(raw: &str, machine: Option<&str>) -> Result<String, Error> {
  if !valid_target(raw) {
    return Err(Error::new("invalid_output", "invalid terminal address"));
  }
  match machine {
    Some(machine) if !raw.contains('/') => Ok(format!("{machine}/{raw}")),
    Some(machine) if !raw.starts_with(&format!("{machine}/")) => {
      Err(Error::new("identity_mismatch", "terminal machine differs"))
    }
    _ => Ok(raw.to_owned()),
  }
}

fn decode(output: &Output) -> Result<Value, Error> {
  if output.timed_out || output.truncated {
    return Err(Error {
      code: "transport".into(),
      message: "hc timed out or output was truncated".into(),
      uncertain: true,
    });
  }
  let parsed = serde_json::from_str::<Value>(&output.stdout)
    .or_else(|_| serde_json::from_str(&output.stderr))
    .map_err(|_error| Error {
      code: "invalid_output".into(),
      message: format!("hc exit {}: invalid JSON", output.exit_code),
      uncertain: true,
    })?;
  if let Some(error) = parsed.get("error") {
    return Err(Error {
      code: error
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned(),
      message: error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("hc failed")
        .to_owned(),
      uncertain: error
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| matches!(kind, "timeout" | "timed_out")),
    });
  }
  if output.exit_code != 0 {
    return Err(uncertain(Error::new(
      "invalid_output",
      format!("hc exit {} with success JSON", output.exit_code),
    )));
  }
  Ok(parsed)
}

#[cfg(test)]
#[path = "tests/hc_test.rs"]
mod tests;
