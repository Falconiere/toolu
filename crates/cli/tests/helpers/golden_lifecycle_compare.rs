//! What the old suites compared: `SessionStart` by exit code, stderr, context and the first
//! line of its title (the runtime line after it is the native binary's); every other hook
//! byte for byte.

use serde::Deserialize;

use crate::golden_lifecycle_cases::{Captured, Case, Res};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SessionOutput {
  hook_specific_output: Option<Context>,
  system_message: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Context {
  additional_context: String,
}

/// The two parts of a `SessionStart` output that bash owned.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Parts {
  pub(crate) context: Option<String>,
  pub(crate) title: String,
}

/// Split a `SessionStart` output into its context and title.
pub(crate) fn session_parts(stdout: &str) -> Res<Parts> {
  let out: SessionOutput = serde_json::from_str(stdout)?;
  let title = out.system_message.unwrap_or_default();
  Ok(Parts {
    context: out
      .hook_specific_output
      .map(|context| context.additional_context),
    title: title.lines().next().unwrap_or("").to_owned(),
  })
}

fn session_diff(got: &Captured, want: &Captured) -> Res<Option<String>> {
  if got.exit_code != want.exit_code || got.stderr != want.stderr {
    return Ok(Some(format!("exit/stderr: got {got:?}, want {want:?}")));
  }
  let (g, w) = (session_parts(&got.stdout)?, session_parts(&want.stdout)?);
  if g != w {
    return Ok(Some(format!(
      "context/title differ\n got: {g:#?}\nwant: {w:#?}"
    )));
  }
  if !got.stdout.ends_with("}\n") {
    return Ok(Some(format!(
      "stdout does not end in a newline: {:?}",
      got.stdout
    )));
  }
  Ok(None)
}

/// A description of how `got` differs from `want` for `case`, or `None` when it matches.
pub(crate) fn diff(case: &Case, got: &Captured, want: &Captured) -> Res<Option<String>> {
  if case.hook == "session-start" {
    return session_diff(got, want);
  }
  if got == want {
    return Ok(None);
  }
  Ok(Some(format!("got {got:#?}\nwant {want:#?}")))
}
