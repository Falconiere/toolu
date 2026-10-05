//! Running the real `toolu` binary cargo built for these tests, and reading
//! what it prints.

use std::error::Error;
use std::process::Output;

use serde_json::Value;

/// A fallible helper result.
pub(crate) type Res<T> = Result<T, Box<dyn Error>>;

/// The `toolu` binary under test.
pub(crate) const TOOLU: &str = env!("CARGO_BIN_EXE_toolu");

/// One run of `toolu` with `args`, its streams captured.
pub(crate) fn toolu(args: &[&str]) -> Res<Output> {
  Ok(assert_cmd::Command::new(TOOLU).args(args).output()?)
}

/// Standard output as text.
pub(crate) fn stdout(output: &Output) -> Res<String> {
  Ok(String::from_utf8(output.stdout.clone())?)
}

/// Standard error as text.
pub(crate) fn stderr(output: &Output) -> Res<String> {
  Ok(String::from_utf8(output.stderr.clone())?)
}

/// Standard output as exactly one JSON document, or an error naming why not.
pub(crate) fn one_document(output: &Output) -> Res<Value> {
  let mut documents = serde_json::Deserializer::from_slice(&output.stdout).into_iter::<Value>();
  let first = documents.next().ok_or("stdout holds no JSON document")??;
  if documents.next().is_some() {
    return Err("stdout holds more than one JSON document".into());
  }
  Ok(first)
}

/// The visible top-level commands, read from `toolu commands --json`.
pub(crate) fn namespaces() -> Res<Vec<String>> {
  let tree = one_document(&toolu(&["commands", "--json"])?)?;
  let commands = tree
    .get("commands")
    .and_then(Value::as_array)
    .ok_or("no commands array")?;
  Ok(
    commands
      .iter()
      .filter(|command| command["hidden"] == false)
      .filter_map(|command| command["name"].as_str().map(str::to_owned))
      .collect(),
  )
}
