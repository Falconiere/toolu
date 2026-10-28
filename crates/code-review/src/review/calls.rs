//! Every network call of a run goes through `Ledger`: it times the call, keeps
//! its usage for the verdict and the usage log, and hands the raw exchange to the
//! eval recorder (#670). A `Call` never holds source, prompts or the key.

use std::time::Instant;

use serde::Serialize;
use serde_json::json;
use toolu_jev_client::{Jev, OPENROUTER_MODEL, Questions, Reply, State, request_body};
use toolu_openrouter::{Client, Completion, StructuredRequest};

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

/// One call, for the usage log: no source, prompt or credential.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Call {
  /// The stage that made it.
  pub stage: Stage,
  /// The model requested or served.
  pub model: String,
  /// The serving provider, when reported.
  pub provider: Option<String>,
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
pub type Recorder<'a> = &'a dyn Fn(&Record<'_>);

/// The calls of one run.
pub(crate) struct Ledger<'a> {
  pub(crate) calls: Vec<Call>,
  recorder: Option<Recorder<'a>>,
}

impl<'a> Ledger<'a> {
  pub(crate) fn new(recorder: Option<Recorder<'a>>) -> Ledger<'a> {
    Ledger {
      calls: Vec::new(),
      recorder,
    }
  }

  /// Ask Jev before `deadline`, recording the call.
  pub(crate) fn jev(
    &mut self,
    step: Stage,
    jev: &Jev,
    (state, questions): (&State, &Questions),
    deadline: Instant,
  ) -> Result<Reply, toolu_jev_client::Error> {
    let started = Instant::now();
    let result = jev.ask_until(state, OPENROUTER_MODEL, questions, deadline);
    let (call, response) = match &result {
      Ok(reply) => (
        Call {
          tokens_in: reply.usage.input_tokens,
          tokens_out: reply.usage.output_tokens,
          cost_usd: reply.usage.cost_usd,
          ..call(step, &reply.model, "ok")
        },
        reply.body.as_str(),
      ),
      Err(err) => (call(step, OPENROUTER_MODEL, &jev_failure(err)), ""),
    };
    self.push(
      call,
      started,
      &request_body(state, OPENROUTER_MODEL, questions),
      response,
    );
    result
  }

  /// Send a structured request before `deadline`, recording the call.
  pub(crate) fn complete(
    &mut self,
    stage: Stage,
    client: &Client,
    request: &StructuredRequest<'_>,
    deadline: Instant,
  ) -> Result<Completion, toolu_openrouter::Error> {
    let started = Instant::now();
    let result = client.complete_structured(request, deadline);
    let messages: Vec<_> = request
      .messages
      .iter()
      .map(|message| json!({"role": message.role, "content": message.content}))
      .collect();
    let sent = json!({"messages": messages, "schema": request.schema_name,
                      "reasoning_effort": request.reasoning_effort, "max_tokens": request.max_tokens});
    let (call, response) = match &result {
      Ok(done) => {
        let usage = &done.usage;
        let response = json!({"content": done.content, "finish_reason": done.finish_reason,
          "model": done.model, "provider": done.provider,
          "usage": {"prompt_tokens": usage.prompt_tokens, "completion_tokens": usage.completion_tokens,
                    "reasoning_tokens": usage.reasoning_tokens, "cost": usage.cost_usd}});
        let call = Call {
          provider: done.provider.clone(),
          tokens_in: usage.prompt_tokens,
          tokens_out: usage.completion_tokens,
          cost_usd: usage.cost_usd,
          ..call(stage, &done.model, "ok")
        };
        (call, response.to_string())
      }
      Err(err) => {
        let failed = call(stage, client.model(), &err.to_string());
        let call = err.usage().map_or(failed.clone(), |(usage, model)| Call {
          model: model.to_owned(),
          tokens_in: usage.prompt_tokens,
          tokens_out: usage.completion_tokens,
          cost_usd: usage.cost_usd,
          ..failed
        });
        (call, String::new())
      }
    };
    self.push(call, started, &sent.to_string(), &response);
    result
  }

  fn push(&mut self, mut call: Call, started: Instant, request: &str, response: &str) {
    call.millis = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    if let Some(recorder) = self.recorder {
      recorder(&Record {
        stage: call.stage,
        request,
        response,
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
    }
  }
}

/// A call that used nothing yet; the caller fills in what it observed.
fn call(stage: Stage, model: &str, outcome: &str) -> Call {
  Call {
    stage,
    model: model.to_owned(),
    provider: None,
    tokens_in: 0,
    tokens_out: 0,
    cost_usd: None,
    millis: 0,
    outcome: outcome.to_owned(),
  }
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
