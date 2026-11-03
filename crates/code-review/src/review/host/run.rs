//! One review call on a host CLI: spawn it, or answer from replay. A deadline
//! returns `Timeout`; any other failure returns an error.

use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "tests/run_test.rs"]
mod tests;

use super::extract::{Extracted, extract};
use super::{Argv, Delivery, HostPlan, argv, delivery, prompt_text};
use crate::review::calls::{Call, Ledger, Stage, call};
use crate::review::{Input, Options, RunError};
use toolu_runtime::env::Env;

/// A live empty review still requires an available host.
pub(crate) fn ensure_host(
  env: &Env,
  input: &Input<'_>,
  options: &Options<'_>,
  ledger: &Ledger<'_>,
) -> Result<(), RunError> {
  if super::available(env.get("PATH")).is_empty()
    && input.shaped.chunks.is_empty()
    && (options.replay.is_none() || options.live_on_miss)
  {
    Err(RunError::Outage {
      message: "no host CLI available".to_owned(),
      usage: ledger.usage(None),
    })
  } else {
    Ok(())
  }
}

/// Why a host review call did not produce findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostError {
  /// The wall deadline elapsed.
  Timeout,
  /// The call was refused before any spawn (no CLI, oversized prompt).
  Refused(String),
  /// The spawn or the output parsing failed.
  Failed(String),
}

/// Try the host CLI: a replayed answer, or a live spawn.
pub(crate) fn complete_via_host(
  ledger: &mut Ledger<'_>,
  stage: Stage,
  plan: &HostPlan,
  request: &toolu_openrouter::StructuredRequest<'_>,
  deadline: Instant,
) -> Result<toolu_openrouter::Completion, toolu_openrouter::Error> {
  let prompt = prompt_text(request.messages);
  let started = Instant::now();
  let remaining = deadline.saturating_duration_since(Instant::now());
  let schema = request.schema.to_string();
  let result = run_review(plan, &schema, &prompt, remaining);
  let outcome = |err: &HostError| match err {
    HostError::Timeout => "deadline".to_owned(),
    HostError::Refused(why) | HostError::Failed(why) => format!("host refused: {why}"),
  };
  let call = match &result {
    Ok((done, _)) => served(stage, done, "ok"),
    Err(err) => call(stage, &plan.model, &outcome(err)),
  };
  push(
    ledger,
    call,
    started,
    request,
    result.as_ref().ok().map(|(done, _)| done),
  );
  match result {
    Ok((done, _)) => Ok(done),
    Err(HostError::Timeout) => Err(toolu_openrouter::Error::Timeout),
    Err(HostError::Refused(_) | HostError::Failed(_)) => {
      Err(toolu_openrouter::Error::InvalidResponse)
    }
  }
}

/// The usage-log call for a served host completion.
fn served(stage: Stage, done: &toolu_openrouter::Completion, outcome: &str) -> Call {
  Call {
    provider: done.provider.clone(),
    quantization: done.quantization.clone(),
    tokens_in: done.usage.prompt_tokens,
    tokens_out: done.usage.completion_tokens,
    cost_usd: done.usage.cost_usd,
    ..call(stage, &done.model, outcome)
  }
}

/// Record a host result in the shared replay format.
fn push(
  ledger: &mut Ledger<'_>,
  call: Call,
  started: Instant,
  request: &toolu_openrouter::StructuredRequest<'_>,
  completion: Option<&toolu_openrouter::Completion>,
) {
  ledger.push(call, started, || {
    (
      crate::review::calls::sent(request, "host"),
      completion.map_or_else(String::new, crate::review::calls::received),
    )
  });
}

/// Run one review call on `plan` and parse its findings JSON. The CLI runs
/// with a temp working directory: the prompt is self-contained evidence, so
/// nothing in the repository is reachable by relative path.
pub(crate) fn run_review(
  plan: &HostPlan,
  schema_json: &str,
  prompt: &str,
  timeout: Duration,
) -> Result<(toolu_openrouter::Completion, String), HostError> {
  if timeout.is_zero() {
    return Err(HostError::Timeout);
  }
  let dir = tempfile::TempDir::new().map_err(|err| HostError::Failed(err.to_string()))?;
  let spec = staged(plan, dir.path(), schema_json, prompt, timeout)?;
  match toolu_runtime::process::run(&spec) {
    Err(err) => Err(HostError::Failed(format!("spawn: {err:?}"))),
    Ok(output) if output.timed_out => Err(HostError::Timeout),
    Ok(output) if output.exit_code != 0 || output.truncated => Err(HostError::Failed(format!(
      "{} exited {} or output was truncated",
      plan.host.name(),
      output.exit_code
    ))),
    Ok(output) => completed(plan, output.stdout),
  }
}

/// The staged CLI invocation: schema and prompt files under `dir`, which the
/// caller keeps alive for the run.
fn staged(
  plan: &HostPlan,
  dir: &std::path::Path,
  schema_json: &str,
  prompt: &str,
  timeout: Duration,
) -> Result<toolu_runtime::process::Spec, HostError> {
  use std::io::Write as _;
  let schema_path = dir.join("schema.json");
  std::fs::write(&schema_path, schema_json).map_err(|err| HostError::Failed(err.to_string()))?;
  let prompt_path = match delivery(plan.host) {
    Delivery::PromptFile => {
      let path = dir.join("prompt.txt");
      let mut file =
        std::fs::File::create(&path).map_err(|err| HostError::Failed(err.to_string()))?;
      file
        .write_all(prompt.as_bytes())
        .map_err(|err| HostError::Failed(err.to_string()))?;
      Some(path)
    }
    Delivery::Stdin | Delivery::Argv => None,
  };
  let bin = plan.bin.to_string_lossy().into_owned();
  let schema = schema_path.to_string_lossy().into_owned();
  let prompt_file = prompt_path
    .as_deref()
    .map(|path| path.to_string_lossy().into_owned());
  let argv = argv(&Argv {
    bin: &bin,
    host: plan.host,
    model: &plan.model,
    schema_file: &schema,
    schema_json,
    prompt,
    prompt_file: prompt_file.as_deref(),
  })
  .map_err(HostError::Refused)?;
  Ok(toolu_runtime::process::Spec {
    argv,
    cwd: Some(dir.to_owned()),
    env: None,
    stdin: match delivery(plan.host) {
      Delivery::Stdin => prompt.as_bytes().to_vec(),
      Delivery::PromptFile | Delivery::Argv => Vec::new(),
    },
    timeout,
    max_output_bytes: toolu_openrouter::MAX_BODY_BYTES,
    wait: toolu_runtime::process::Wait::Group,
    poll_interval: Duration::from_millis(10),
  })
}

/// The completion for the CLI's `stdout`, or why it carries no findings.
fn completed(
  plan: &HostPlan,
  stdout: String,
) -> Result<(toolu_openrouter::Completion, String), HostError> {
  match extract(plan.host, &stdout) {
    Ok(found) => Ok(completion(plan, found, stdout)),
    Err(why) => Err(HostError::Failed(format!("{}: {why}", plan.host.name()))),
  }
}

/// The completion for extracted host text paired with its raw stdout.
fn completion(
  plan: &HostPlan,
  found: Extracted,
  raw: String,
) -> (toolu_openrouter::Completion, String) {
  (
    toolu_openrouter::Completion {
      content: found.text,
      finish_reason: Some("stop".to_owned()),
      model: plan.model.clone(),
      provider: Some(plan.host.name().to_owned()),
      quantization: None,
      usage: toolu_openrouter::Usage {
        prompt_tokens: found.tokens_in,
        completion_tokens: found.tokens_out,
        reasoning_tokens: 0,
        cost_usd: found.cost_usd,
      },
    },
    raw,
  )
}
