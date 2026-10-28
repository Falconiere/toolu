//! What a replayed call needs besides the ledger: the lookup type, the outcome labels
//! a replayed or missed call carries, and the completion rebuilt from the JSON the
//! recorder stored (#670).

use serde_json::Value;
use toolu_openrouter::{Completion, Usage as Observed};

use super::calls::Stage;

/// Looks up the recorded response to a stage's exact masked request text (#670).
/// With a lookup in place a run never reaches the network: a hit is replayed and a
/// miss fails that call closed, unless `Options::live_on_miss` lets it through.
pub type Replay<'a> = &'a dyn Fn(Stage, &str) -> Option<String>;

/// The `Call::outcome` of a call answered from the replay store.
pub(crate) const REPLAYED: &str = "replayed";

/// The `Call::outcome` of a call whose request the replay store did not hold.
pub(crate) const REPLAY_MISS: &str = "replay miss";

/// A completion rebuilt from the JSON `received` recorded, charged nothing.
pub(super) fn restored(body: &str) -> Result<Completion, toolu_openrouter::Error> {
  let doc: Value =
    serde_json::from_str(body).map_err(|_err| toolu_openrouter::Error::InvalidResponse)?;
  let text = |key: &str| doc.get(key).and_then(Value::as_str).map(str::to_owned);
  let count = |key: &str| {
    doc
      .get("usage")
      .and_then(|usage| usage.get(key))
      .and_then(Value::as_u64)
  };
  Ok(Completion {
    content: text("content").ok_or(toolu_openrouter::Error::InvalidResponse)?,
    finish_reason: text("finish_reason"),
    model: text("model").ok_or(toolu_openrouter::Error::InvalidResponse)?,
    provider: text("provider"),
    quantization: text("provider_quantization"),
    usage: Observed {
      prompt_tokens: count("prompt_tokens").ok_or(toolu_openrouter::Error::InvalidResponse)?,
      completion_tokens: count("completion_tokens")
        .ok_or(toolu_openrouter::Error::InvalidResponse)?,
      reasoning_tokens: count("reasoning_tokens").unwrap_or(0),
      cost_usd: Some(0.0),
    },
  })
}

/// Why a replayed structured call failed, in a few words.
pub(super) fn replay_failure(err: &toolu_openrouter::Error) -> String {
  if matches!(err, toolu_openrouter::Error::InvalidConfig) {
    REPLAY_MISS.to_owned()
  } else {
    err.to_string()
  }
}

#[cfg(test)]
#[path = "tests/replayed_test.rs"]
mod tests;
