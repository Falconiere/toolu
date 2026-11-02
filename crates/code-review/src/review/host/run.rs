//! One review call on a host CLI: spawn it, or answer from replay. A deadline
//! returns `Timeout` without fallback; any other failure returns an error the
//! caller falls through to `OpenRouter` with.

use std::time::{Duration, Instant};

#[cfg(test)]
#[path = "tests/run_test.rs"]
mod tests;

use super::extract::{Extracted, extract};
use super::{Argv, Delivery, HostPlan, argv, delivery, prompt_text};
use crate::review::calls::{Call, Ledger, Stage, call};
use crate::review::replayed::{REPLAY_MISS, REPLAYED};

/// Why a host review call did not produce findings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HostError {
  /// The wall deadline elapsed; never falls back, like the `OpenRouter` path.
  Timeout,
  /// The call was refused before any spawn (no CLI, oversized prompt).
  Refused(String),
  /// The spawn or the output parsing failed; the caller falls back.
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
  if let Some(replay) = ledger.replay {
    let body = replay(stage, &prompt);
    if body.is_some() || !ledger.live_on_miss {
      return replayed_host(ledger, body, stage, plan, &prompt);
    }
  }
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
    &prompt,
    result.as_ref().ok().map(|(_, stdout)| stdout.as_str()),
  );
  match result {
    Ok((done, _)) => Ok(done),
    Err(HostError::Timeout) => Err(toolu_openrouter::Error::Timeout),
    Err(HostError::Refused(_) | HostError::Failed(_)) => {
      Err(toolu_openrouter::Error::InvalidResponse)
    }
  }
}

/// Answer a host review call from `replay`: no CLI is spawned.
fn replayed_host(
  ledger: &mut Ledger<'_>,
  body: Option<String>,
  stage: Stage,
  plan: &HostPlan,
  prompt: &str,
) -> Result<toolu_openrouter::Completion, toolu_openrouter::Error> {
  let started = Instant::now();
  let result: Result<(toolu_openrouter::Completion, String), toolu_openrouter::Error> = match body {
    Some(body) => extract(plan.host, &body)
      .map(|found| completion(plan, found, body))
      .map_err(|_err| toolu_openrouter::Error::InvalidResponse),
    None => Err(toolu_openrouter::Error::InvalidConfig),
  };
  let call = match &result {
    Ok((done, _)) => served(stage, done, REPLAYED),
    Err(_) => call(stage, &plan.model, REPLAY_MISS),
  };
  push(
    ledger,
    call,
    started,
    prompt,
    result.as_ref().ok().map(|(_, stdout)| stdout.as_str()),
  );
  result.map(|(done, _)| done)
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

/// Log `call` with the prompt and the CLI's stdout as the recorded exchange.
fn push(ledger: &mut Ledger<'_>, call: Call, started: Instant, prompt: &str, stdout: Option<&str>) {
  let response = stdout.unwrap_or_default().to_owned();
  ledger.push(call, started, || (prompt.to_owned(), response));
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
      finish_reason: None,
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
