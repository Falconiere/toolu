//! Review through host CLI sub-agents instead of `OpenRouter`.
//!
//! Jev picks the host, the model, and the effort; every choice has a
//! deterministic fallback, and any transport failure falls back to the
//! `OpenRouter` client. Prompts never travel in argv: `claude` and `codex`
//! read them from stdin, `muse` from a prompt file, and the argv-only hosts
//! refuse oversized prompts.

use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};
use toolu_jev_client::{Answer, Jev, Question, Questions, State};
use toolu_protocol::host::Host;
use toolu_runtime::env::Env;

use super::calls::{Ledger, Stage};
use super::effort;
use super::{Input, Options};

#[cfg(test)]
#[path = "tests/host_test.rs"]
mod tests;

pub(crate) mod extract;
pub(crate) mod run;

/// Above this prompt size the argv-only hosts refuse the call: argv cannot
/// carry a review prompt, and secrets must not appear in a process listing.
// shortcut: unlink argv delivery once every host reads stdin or a file.
pub(crate) const MAX_ARGV_PROMPT_BYTES: usize = 90_000;

/// How the prompt reaches the host CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Delivery {
  /// Piped stdin (`claude -p`, `codex exec -`).
  Stdin,
  /// A prompt file (`muse exec --prompt-file`).
  PromptFile,
  /// A prompt argument, size-capped (`cursor-agent`, `opencode run`).
  Argv,
}

/// The Jev-selected review transport: `"default"` model omits the model flag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostPlan {
  /// The host whose CLI runs the review.
  pub(crate) host: Host,
  /// A model id, or `"default"`.
  pub(crate) model: String,
  /// The resolved CLI binary: probed at selection, spawned without re-search.
  pub(crate) bin: PathBuf,
}

/// No model is ever the default: the flag is omitted and the CLI resolves it.
pub(crate) const DEFAULT_MODEL: &str = "default";

/// The CLI binary, when the host has one. `cursor` alone is an IDE stub, so
/// its agent binary is used; Hermes documents no CLI.
pub(crate) fn binary(host: Host) -> Option<&'static str> {
  match host {
    Host::Claude => Some("claude"),
    Host::Codex => Some("codex"),
    Host::Cursor => Some("cursor-agent"),
    Host::Hermes => None,
    Host::Muse => Some("muse"),
    Host::Opencode => Some("opencode"),
  }
}

/// Hosts whose CLI binary is executable somewhere in `path`.
pub(crate) fn available(path: Option<&str>) -> Vec<Host> {
  let Some(path) = path else { return Vec::new() };
  Host::ALL
    .into_iter()
    .filter(|host| resolve(*host, path).is_some())
    .collect()
}

/// The executable CLI for `host` in `path`, when it has one.
pub(crate) fn resolve(host: Host, path: &str) -> Option<PathBuf> {
  binary(host).and_then(|name| find_on_path(name, path))
}

/// How the prompt reaches `host`, pinned by the CLI spike.
pub(crate) fn delivery(host: Host) -> Delivery {
  match host {
    Host::Claude | Host::Codex => Delivery::Stdin,
    Host::Muse => Delivery::PromptFile,
    Host::Cursor | Host::Opencode | Host::Hermes => Delivery::Argv,
  }
}

/// The pinned non-default model, when the spike verified one. Model catalogs
/// drift, so entries here are re-pinned by hand; a stale id fails the spawn
/// and the run falls back to `OpenRouter`.
// shortcut: refresh from `cursor-agent --list-models` / `opencode models` when a run falls back on an unknown model id.
pub(crate) fn premium_model(host: Host) -> Option<&'static str> {
  match host {
    Host::Cursor => Some("claude-opus-5-thinking-high"),
    Host::Opencode => Some("anthropic/claude-opus-5"),
    Host::Claude | Host::Codex | Host::Hermes | Host::Muse => None,
  }
}

/// The Jev model-choice options: always `"default"`, plus the premium id.
pub(crate) fn model_options(host: Host) -> Vec<(&'static str, &'static str)> {
  let mut options = vec![(DEFAULT_MODEL, "The CLI's configured default model")];
  if let Some(premium) = premium_model(host) {
    options.push((premium, "The pinned strong model for hard diffs"));
  }
  options
}

/// One host CLI invocation: everything `argv` needs.
pub(crate) struct Argv<'a> {
  pub(crate) bin: &'a str,
  pub(crate) host: Host,
  pub(crate) model: &'a str,
  pub(crate) schema_file: &'a str,
  pub(crate) prompt: &'a str,
  pub(crate) prompt_file: Option<&'a str>,
}

/// The argv for one review call. `schema_file` carries the findings schema
/// for the hosts that enforce it; `prompt_file` carries the prompt for
/// `muse`; every other host takes the prompt by stdin or, size-capped, argv.
pub(crate) fn argv(spec: &Argv<'_>) -> Result<Vec<String>, String> {
  let host = spec.host;
  if binary(host).is_none() {
    return Err(format!("{} has no review CLI", host.name()));
  }
  if delivery(host) == Delivery::Argv && spec.prompt.len() > MAX_ARGV_PROMPT_BYTES {
    return Err(format!(
      "{} takes the prompt as an argument, but {} bytes do not fit",
      host.name(),
      spec.prompt.len()
    ));
  }
  let mut args = vec![spec.bin.to_owned()];
  args.extend(flags(spec)?);
  if spec.model != DEFAULT_MODEL {
    if spec.model.trim().is_empty() || spec.model.contains(char::is_whitespace) {
      return Err(format!("refusing model id {:?}", spec.model));
    }
    args.extend(["--model".to_owned(), spec.model.to_owned()]);
  }
  if delivery(host) == Delivery::Argv {
    args.push(spec.prompt.to_owned());
  }
  Ok(args)
}

/// The host-specific flags; the binary, the model flag, and the prompt itself
/// stay in `argv`.
fn flags(spec: &Argv<'_>) -> Result<Vec<String>, String> {
  let host = spec.host;
  let mut args = Vec::new();
  match host {
    Host::Claude => {
      args.extend(["-p".into(), "--output-format".into(), "json".into()]);
      args.extend(["--json-schema".into(), spec.schema_file.into()]);
    }
    Host::Codex => {
      args.extend([
        "exec".into(),
        "-s".into(),
        "read-only".into(),
        "--skip-git-repo-check".into(),
        "--output-schema".into(),
        spec.schema_file.into(),
        "-".into(),
      ]);
    }
    Host::Cursor => {
      args.extend([
        "-p".into(),
        "--trust".into(),
        "--output-format".into(),
        "json".into(),
      ]);
    }
    Host::Muse => {
      let prompt_file = spec
        .prompt_file
        .ok_or_else(|| "muse takes the prompt from a file".to_owned())?;
      args.extend([
        "exec".into(),
        "--prompt-file".into(),
        prompt_file.into(),
        "--output-schema".into(),
        spec.schema_file.into(),
      ]);
    }
    Host::Opencode => {
      args.extend(["run".into(), "--format".into(), "json".into()]);
    }
    Host::Hermes => return Err("hermes has no review CLI".to_owned()),
  }
  Ok(args)
}

/// The reviewed host: Jev's answer when it names an available host, else the
/// default when available, else the first available host, else none.
pub(crate) fn pick_host(available: &[Host], answer: Option<&str>, default: Host) -> Option<Host> {
  if let Some(host) = answer.and_then(Host::parse)
    && available.contains(&host)
  {
    return Some(host);
  }
  if available.contains(&default) {
    return Some(default);
  }
  available.first().copied()
}

/// The model id: the premium id only when Jev named it, else `"default"`.
pub(crate) fn pick_model(host: Host, answer: Option<&str>) -> String {
  if answer.is_some_and(|name| Some(name) == premium_model(host)) {
    return answer.unwrap_or(DEFAULT_MODEL).to_owned();
  }
  DEFAULT_MODEL.to_owned()
}

/// Find `binary` in `path`, executable.
pub(crate) fn find_on_path(binary: &str, path: &str) -> Option<PathBuf> {
  use std::os::unix::fs::PermissionsExt as _;
  path.split(':').find_map(|dir| {
    if dir.is_empty() {
      return None;
    }
    let candidate = PathBuf::from(dir).join(binary);
    let executable = std::fs::metadata(&candidate)
      .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0);
    executable.then_some(candidate)
  })
}

/// The host-choice question over `state.hosts` (shape metadata plus hosts).
pub(crate) const HOST_QUESTION: &str = "Which available host CLI in `state.hosts` should review the change described in `state.shape`? Prefer the host already running this review when listed; otherwise prefer a host with native schema output.";

/// The model-choice question for the selected host.
pub(crate) const MODEL_QUESTION: &str = "Which model in `state.models` should review the change? Pick `default` unless the change needs the strongest reasoning.";

/// The Jev host-choice question over `available`.
pub(crate) fn host_question(available: &[Host]) -> Result<toolu_jev_client::Question, String> {
  let options: Vec<(&str, Option<&str>)> = available
    .iter()
    .map(|host| {
      (
        host.name(),
        Some(if delivery(*host) == Delivery::Argv {
          "Review CLI without schema output; prompts are size-capped"
        } else {
          "Review CLI with native schema output"
        }),
      )
    })
    .collect();
  toolu_jev_client::Question::choice(HOST_QUESTION, &options).map_err(|err| err.to_string())
}

/// The Jev model-choice question for `host`.
pub(crate) fn model_question(host: Host) -> Result<toolu_jev_client::Question, String> {
  let options: Vec<(&str, Option<&str>)> = model_options(host)
    .into_iter()
    .map(|(key, about)| (key, Some(about)))
    .collect();
  toolu_jev_client::Question::choice(MODEL_QUESTION, &options).map_err(|err| err.to_string())
}

/// One prompt text from chat messages; hosts take no roles.
pub(crate) fn prompt_text(messages: &[toolu_openrouter::Message<'_>]) -> String {
  messages
    .iter()
    .map(|message| format!("{}: {}", message.role, message.content))
    .collect::<Vec<_>>()
    .join("\n\n")
}

/// The effort selector's own wall deadline.
pub(crate) const SELECTOR_DEADLINE: std::time::Duration = std::time::Duration::from_secs(60);

/// The Jev-selected host transport, or `None` for `OpenRouter` only. Replayed
/// runs never spawn a CLI, and neither do runs with nothing to review; without
/// an available CLI there is nothing to select.
pub(crate) fn select_host(
  env: &Env,
  input: &Input<'_>,
  options: &Options<'_>,
  jev: &Jev,
  ledger: &mut Ledger<'_>,
) -> Option<HostPlan> {
  if options.replay.is_some() && !options.live_on_miss {
    return None;
  }
  if input.shaped.chunks.is_empty() {
    return None;
  }
  let available_hosts = available(env.get("PATH"));
  if available_hosts.is_empty() {
    return None;
  }
  let default = toolu_runtime::host::detect::detect(env, None).host;
  let path = env.get("PATH").unwrap_or("");
  let plan =
    |host: Host, model: String| resolve(host, path).map(|bin| HostPlan { host, model, bin });
  if options.skip_judge {
    return pick_host(&available_hosts, None, default)
      .and_then(|host| plan(host, DEFAULT_MODEL.to_owned()));
  }
  let shape = effort::metadata(input.shaped, input.prepass).to_string();
  let names: Vec<&str> = available_hosts.iter().map(|host| host.name()).collect();
  let state = State::structured(
    &json!({"shape": serde_json::from_str::<Value>(&shape).unwrap_or(Value::Null), "hosts": names})
      .to_string(),
  );
  let answer = (available_hosts.len() > 1)
    .then(|| {
      host_question(&available_hosts)
        .ok()
        .and_then(|question| ask_choice(ledger, jev, &state, "host", question))
    })
    .flatten();
  let picked = pick_host(&available_hosts, answer.as_deref(), default)?;
  let model = ask_model(picked, ledger, jev);
  plan(picked, model)
}

/// The Jev-selected model for `picked`, or the default without asking.
fn ask_model(picked: Host, ledger: &mut Ledger<'_>, jev: &Jev) -> String {
  let models: Vec<&str> = model_options(picked)
    .into_iter()
    .map(|(key, _)| key)
    .collect();
  let answer = (models.len() > 1)
    .then(|| {
      let state = State::structured(&json!({"host": picked.name(), "models": models}).to_string());
      model_question(picked)
        .ok()
        .and_then(|question| ask_choice(ledger, jev, &state, "model", question))
    })
    .flatten();
  pick_model(picked, answer.as_deref())
}

/// One Jev choice answer, `None` for any failure or a different answer kind.
fn ask_choice(
  ledger: &mut Ledger<'_>,
  jev: &Jev,
  state: &State,
  id: &str,
  question: Question,
) -> Option<String> {
  let reply = ledger
    .jev(
      Stage::Selector,
      jev,
      (state, &Questions::single(id, question)),
      Instant::now() + SELECTOR_DEADLINE,
    )
    .ok()?;
  match reply.answers.first() {
    Some((_, Answer::Choice { choice, .. })) => Some(choice.clone()),
    Some(_) | None => None,
  }
}
