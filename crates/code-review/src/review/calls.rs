//! Every network call of a run goes through `Ledger`: it times the call, keeps
//! its usage for the verdict and the usage log, and hands the raw exchange to the
//! eval recorder (#670). A `Call` never holds source, prompts or the key.

use std::time::Instant;

use serde::Serialize;
use serde_json::json;
use toolu_jev_client::{DEFAULT_MODEL, Jev, Questions, Reply, State, parse_reply, request_body};
use toolu_openrouter::{Completion, StructuredRequest};

use super::host::{HostPlan, run};
use super::replayed::{REPLAY_MISS, REPLAYED, Replay, replay_failure, restored};
use crate::contract::{Cost, Usage};

/// Which stage made a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Stage {
  /// Jev choosing the effort level.
  Selector,
  /// The structured review of one chunk.
  Review,
  /// Jev judging findings.
  Judge,
  /// The generative recheck of contradicted findings.
  Recheck,
}

impl Stage {
  /// The lowercase name the usage log and the replay store use.
  pub fn name(self) -> &'static str {
    match self {
      Stage::Selector => "selector",
      Stage::Review => "review",
      Stage::Judge => "judge",
      Stage::Recheck => "recheck",
    }
  }
}

/// One call, for the usage log: no source, prompt or credential.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Call {
  /// The stage that made it.
  pub stage: Stage,
  /// The model requested or served.
  pub model: String,
  /// The serving provider, when reported.
  pub provider: Option<String>,
  /// Serving quantization, when reported.
  pub quantization: Option<String>,
  /// Input tokens.
  pub tokens_in: u64,
  /// Output tokens, reasoning included.
  pub tokens_out: u64,
  /// The reported cost, when there was one.
  pub cost_usd: Option<f64>,
  /// Wall time in milliseconds.
  pub millis: u64,
  /// `ok`, or the failure's description.
  pub outcome: String,
}

/// One raw exchange for the eval recorder: the masked request and the response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Record<'a> {
  /// The stage that made it.
  pub stage: Stage,
  /// The request body as sent (Jev) or its messages and settings (review calls).
  pub request: &'a str,
  /// The Jev reply body, or the completion as JSON; empty when the call failed.
  pub response: &'a str,
}

/// Receives every exchange of a run.
pub(crate) type Recorder<'a> = &'a dyn Fn(&Record<'_>);

/// One outbound structured call: where it goes and what it carries.
pub(crate) struct Outbound<'a, 'q, 'r> {
  /// The stage making the call.
  pub(crate) stage: Stage,
  /// The structured request.
  pub(crate) request: &'r StructuredRequest<'q>,
  /// The call's wall deadline.
  pub(crate) deadline: Instant,
  /// The Jev-selected host transport, when live and available.
  pub(crate) host: Option<&'a HostPlan>,
}

/// The calls of one run.
pub(crate) struct Ledger<'a> {
  pub(crate) calls: Vec<Call>,
  recorder: Option<Recorder<'a>>,
  pub(crate) replay: Option<Replay<'a>>,
  pub(crate) live_on_miss: bool,
}

impl<'a> Ledger<'a> {
  pub(crate) fn new(recorder: Option<Recorder<'a>>) -> Ledger<'a> {
    Ledger {
      calls: Vec::new(),
      recorder,
      replay: None,
      live_on_miss: false,
    }
  }

  /// Answer every call from `replay`, before any network call, when it is set.
  pub(crate) fn replaying(mut self, replay: Option<Replay<'a>>) -> Ledger<'a> {
    self.replay = replay;
    self
  }

  /// Send a request the replay lookup does not hold instead of failing it closed.
  pub(crate) fn live_on_miss(mut self, live: bool) -> Ledger<'a> {
    self.live_on_miss = live;
    self
  }

  /// Ask Jev before `deadline`, recording the call.
  pub(crate) fn jev(
    &mut self,
    step: Stage,
    jev: &Jev,
    (state, questions): (&State, &Questions),
    deadline: Instant,
  ) -> Result<Reply, toolu_jev_client::Error> {
    if let Some(replay) = self.replay {
      let body = replay(step, &request_body(state, DEFAULT_MODEL, questions));
      if body.is_some() || !self.live_on_miss {
        return self.replayed_jev(body, step, questions);
      }
    }
    let started = Instant::now();
    let result = jev.ask_until(state, DEFAULT_MODEL, questions, deadline);
    let call = match &result {
      Ok(reply) => Call {
        tokens_in: reply.usage.input_tokens,
        tokens_out: reply.usage.output_tokens,
        cost_usd: reply.usage.cost_usd,
        ..call(step, &reply.model, "ok")
      },
      Err(err) => call(step, DEFAULT_MODEL, &jev_failure(err)),
    };
    self.push(call, started, || {
      let response = result.as_ref().map_or("", |reply| reply.body.as_str());
      (
        request_body(state, DEFAULT_MODEL, questions),
        response.to_owned(),
      )
    });
    result
  }

  /// Send a structured request to the host CLI before `deadline`, or replay it.
  pub(crate) fn complete(
    &mut self,
    send: &Outbound<'_, '_, '_>,
  ) -> Result<Completion, toolu_openrouter::Error> {
    if let Some(done) = self.replayed(send) {
      return done;
    }
    if let Some(plan) = send.host {
      run::complete_via_host(self, send.stage, plan, send.request, send.deadline)
    } else {
      self.push(
        call(send.stage, "host", "no host CLI available"),
        Instant::now(),
        || (sent(send.request, "host"), String::new()),
      );
      Err(toolu_openrouter::Error::InvalidResponse)
    }
  }

  /// The replayed answer, when the store holds the request or a
  /// miss must fail closed: `None` sends live instead.
  fn replayed(
    &mut self,
    send: &Outbound<'_, '_, '_>,
  ) -> Option<Result<Completion, toolu_openrouter::Error>> {
    let replay = self.replay?;
    let body = replay(send.stage, &sent(send.request, "host"));
    (body.is_some() || !self.live_on_miss).then(|| self.replayed_complete(body, send.stage))
  }

  /// Log `call`; the recorder, when there is one, gets the `exchange` texts.
  pub(crate) fn push(
    &mut self,
    mut call: Call,
    started: Instant,
    exchange: impl FnOnce() -> (String, String),
  ) {
    call.millis = elapsed(started);
    if let Some(recorder) = self.recorder {
      let (request, response) = exchange();
      recorder(&Record {
        stage: call.stage,
        request: &request,
        response: &response,
      });
    }
    self.calls.push(call);
  }

  /// Tokens and reported costs summed over every call; the cost is unknown only
  /// when no call reported one.
  pub(crate) fn usage(&self, model: Option<String>) -> Usage {
    let reported: Vec<f64> = self.calls.iter().filter_map(|call| call.cost_usd).collect();
    Usage {
      tokens_in: self.calls.iter().map(|call| call.tokens_in).sum(),
      tokens_out: self.calls.iter().map(|call| call.tokens_out).sum(),
      cost_usd: (!reported.is_empty())
        .then(|| reported.iter().sum())
        .and_then(Cost::usd),
      model,
      calls: self.calls.clone(),
    }
  }
}

/// Replayed answers: no request is sent and nothing is charged.
impl Ledger<'_> {
  /// Answer a Jev question from its replayed `body`, `None` for a miss: no request is sent and nothing is charged.
  fn replayed_jev(
    &mut self,
    body: Option<String>,
    step: Stage,
    questions: &Questions,
  ) -> Result<Reply, toolu_jev_client::Error> {
    let started = Instant::now();
    let result = match body {
      Some(body) => parse_reply(&body, questions),
      None => Err(toolu_jev_client::Error::InvalidQuestion(
        REPLAY_MISS.to_owned(),
      )),
    };
    let call = match &result {
      Ok(answer) => Call {
        tokens_in: answer.usage.input_tokens,
        tokens_out: answer.usage.output_tokens,
        cost_usd: Some(0.0),
        ..call(step, &answer.model, REPLAYED)
      },
      Err(err) => call(step, DEFAULT_MODEL, &jev_failure(err)),
    };
    self.calls.push(Call {
      millis: elapsed(started),
      ..call
    });
    result
  }

  /// Answer a structured request from `replay`: no request is sent and nothing is
  /// charged.
  fn replayed_complete(
    &mut self,
    body: Option<String>,
    stage: Stage,
  ) -> Result<Completion, toolu_openrouter::Error> {
    let started = Instant::now();
    let result = match body {
      Some(body) => restored(&body),
      None => Err(toolu_openrouter::Error::InvalidConfig),
    };
    let call = match &result {
      Ok(done) => Call {
        provider: done.provider.clone(),
        quantization: done.quantization.clone(),
        tokens_in: done.usage.prompt_tokens,
        tokens_out: done.usage.completion_tokens,
        cost_usd: done.usage.cost_usd,
        ..call(stage, &done.model, REPLAYED)
      },
      Err(err) => call(stage, "host", &replay_failure(err)),
    };
    self.calls.push(Call {
      millis: elapsed(started),
      ..call
    });
    result
  }
}

/// Whole milliseconds since `started`.
fn elapsed(started: Instant) -> u64 {
  u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// A call that used nothing yet; the caller fills in what it observed.
pub(crate) fn call(stage: Stage, model: &str, outcome: &str) -> Call {
  Call {
    stage,
    model: model.to_owned(),
    provider: None,
    quantization: None,
    tokens_in: 0,
    tokens_out: 0,
    cost_usd: None,
    millis: 0,
    outcome: outcome.to_owned(),
  }
}

/// A structured request's model, messages and settings, as the recorder sees them;
/// the model is part of the replay key.
pub(super) fn sent(request: &StructuredRequest<'_>, model: &str) -> String {
  let messages: Vec<_> = request
    .messages
    .iter()
    .map(|message| json!({"role": message.role, "content": message.content}))
    .collect();
  json!({"model": model, "messages": messages, "schema": request.schema_name,
         "reasoning_effort": request.reasoning_effort, "max_tokens": request.max_tokens})
  .to_string()
}

/// A completion as JSON, as the recorder sees it.
pub(super) fn received(done: &Completion) -> String {
  let usage = &done.usage;
  json!({"content": done.content, "finish_reason": done.finish_reason,
    "model": done.model, "provider": done.provider, "provider_quantization": done.quantization,
    "usage": {"prompt_tokens": usage.prompt_tokens, "completion_tokens": usage.completion_tokens,
              "reasoning_tokens": usage.reasoning_tokens, "cost": usage.cost_usd}})
  .to_string()
}

/// A Jev failure in a few words, without its response body.
pub(crate) fn jev_failure(err: &toolu_jev_client::Error) -> String {
  match err {
    toolu_jev_client::Error::Http { status, .. } => format!("HTTP {status}"),
    toolu_jev_client::Error::Timeout => "deadline".to_owned(),
    toolu_jev_client::Error::Transport(_) => "transport failed".to_owned(),
    toolu_jev_client::Error::InvalidResponse => "invalid response".to_owned(),
    toolu_jev_client::Error::MissingKey
    | toolu_jev_client::Error::KeyLineBreak
    | toolu_jev_client::Error::InvalidQuestion(_) => err.to_string(),
  }
}

#[cfg(test)]
#[path = "tests/calls_test.rs"]
mod tests;
