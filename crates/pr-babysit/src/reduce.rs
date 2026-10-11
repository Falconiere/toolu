//! The pure decision layer: a snapshot, the previous state and the time give
//! the next state and the tick result. No network, no clock, no backoff.

use serde_json::Value;

use crate::json::{array, jq_string, nil, or, text};

mod attention;
mod docs;
mod inputs;
mod reasons;
pub(crate) mod threads;

use attention::Work;
use inputs::{Bot, Memory};
use reasons::Facts;

/// The next state and the result of one tick.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Reduced {
  /// The next version-2 state.
  pub(crate) state: Value,
  /// The version-1 result.
  pub(crate) result: Value,
}

/// Where the state, snapshot and result files live.
pub(crate) struct Paths<'a> {
  /// The state file.
  pub(crate) state: &'a str,
  /// The snapshot file.
  pub(crate) snapshot: &'a str,
  /// The result file.
  pub(crate) result: &'a str,
}

/// Everything one tick derived, which the documents are written from.
struct Tick<'a> {
  snapshot: &'a Value,
  previous: Option<&'a Value>,
  now: &'a str,
  paths: &'a Paths<'a>,
  bot: Bot,
  memory: Memory,
  checks: Vec<Value>,
  ci: &'static str,
  threads: threads::Threads,
  conversation: (Vec<Value>, Vec<Value>),
  reviews: (Vec<Value>, Vec<Value>),
  recurring: Vec<Value>,
  streak: u64,
  changed: bool,
  reasons: Vec<Value>,
  decision: &'static str,
  attention: Value,
}

fn changed(previous: Option<&Value>, comparison: &[(&str, Value); 8]) -> bool {
  let Some(before) = previous.and_then(|prev| nil(prev.get("pr"))) else {
    return true;
  };
  comparison
    .iter()
    .any(|(key, now)| before.get(*key).unwrap_or(&Value::Null) != now)
}

fn fixer_detail(memory: &Memory) -> Option<String> {
  let in_flight = memory.fixing.as_ref()?.len();
  let fixer = &memory.fixer;
  Some(format!(
    "fixer {}: group {} of {}; {in_flight} item(s) in flight",
    text(fixer, "status"),
    jq_string(&or(fixer.get("current"), Value::from(1))),
    array(fixer.get("groups")).len()
  ))
}

fn head(snapshot: &Value) -> &str {
  snapshot
    .pointer("/head/sha")
    .and_then(Value::as_str)
    .unwrap_or("")
}

/// The finding keys a new verdict run repeats from the last round, and the
/// recurrence streak they make.
fn recurrence(bot: &Bot, memory: &Memory, has_previous: bool) -> (Vec<Value>, u64) {
  if bot.same_run {
    return (Vec::new(), memory.streak);
  }
  let recurring: Vec<Value> = if has_previous {
    let last = &memory.last_round_keys;
    bot
      .keys
      .iter()
      .filter(|key| last.contains(key))
      .cloned()
      .collect()
  } else {
    Vec::new()
  };
  let streak = if recurring.is_empty() {
    0
  } else {
    memory.streak + 1
  };
  (recurring, streak)
}

/// Reduce one snapshot against the previous state.
pub(crate) fn reduce(
  snapshot: &Value,
  previous: Option<&Value>,
  now: &str,
  paths: &Paths<'_>,
) -> Reduced {
  let pr = snapshot.get("pr").unwrap_or(&Value::Null);
  let author = pr.get("author").unwrap_or(&Value::Null);
  let bot = inputs::bot(snapshot, previous);
  let memory = inputs::memory(previous);
  let checks = inputs::checks(pr);
  let ci = inputs::ci_status(&checks);
  let owned = |item: &Value| memory.owns(item);
  let threads = threads::threads(snapshot, author, &memory.actions, &owned);
  let replied = memory.actions.get("replied");
  let conversation = threads::conversation(snapshot, author, replied, &owned);
  let reviews = threads::reviews(snapshot, author, replied, &owned);
  let (recurring, streak) = recurrence(&bot, &memory, previous.is_some());
  let comparison = docs::comparison(pr, ci, threads.unresolved, head(snapshot), &bot);
  let mut tick = Tick {
    snapshot,
    previous,
    now,
    paths,
    changed: changed(previous, &comparison),
    bot,
    memory,
    checks,
    ci,
    threads,
    conversation,
    reviews,
    recurring,
    streak,
    reasons: Vec::new(),
    decision: "keep_going",
    attention: Value::Null,
  };
  judge(&mut tick, previous);
  Reduced {
    state: docs::state(&tick),
    result: docs::result(&tick),
  }
}

fn facts<'a>(tick: &'a Tick<'_>, previous: Option<&Value>) -> Facts<'a> {
  let pr = tick.snapshot.get("pr").unwrap_or(&Value::Null);
  let head = head(tick.snapshot);
  let prev_pr = previous.and_then(|prev| prev.get("pr"));
  let prev_text = |key: &str| prev_pr.and_then(|pr| pr.get(key)).and_then(Value::as_str);
  let bot = &tick.bot;
  Facts {
    pr_state: text(pr, "state"),
    mergeable: text(pr, "mergeable"),
    ci: tick.ci,
    checks: &tick.checks,
    bot_state: &bot.state,
    bot_verdict: &bot.label,
    findings: bot.keys.len(),
    degraded: bot.degraded,
    bot_url: nil(bot.comment.get("url")).and_then(Value::as_str),
    head,
    fix_attempts: tick.memory.fix_attempts,
    recurring: tick.recurring.len(),
    had_rejection: tick.memory.had_rejection,
    streak: tick.streak,
    provider_repeated: previous.is_some()
      && !bot.same_run
      && bot.state == "provider_error"
      && prev_text("botState") == Some("provider_error")
      && prev_text("headSha") == Some(head),
    unresolved: tick.threads.unresolved,
    actionable: tick.threads.actionable.len(),
    stale: tick.threads.stale.len(),
    fixer: fixer_detail(&tick.memory),
    changed: tick.changed,
  }
}

fn judge(tick: &mut Tick<'_>, previous: Option<&Value>) {
  let head = head(tick.snapshot);
  let bot = &tick.bot;
  let facts = facts(tick, previous);
  let escalations = facts.escalations();
  let decision = facts.decision(!escalations.is_empty());
  let work = Work {
    escalations: &escalations,
    ci: tick.ci,
    checks: &tick.checks,
    head,
    threads: &tick.threads.actionable,
    stale: &tick.threads.stale,
    conversation: &tick.conversation.0,
    reviews: &tick.reviews.0,
    review_changes: !bot.same_run && bot.state == "complete" && !facts.approved(),
    provider_error: !bot.same_run && bot.state == "provider_error",
    bot_run: format!(
      "{}@{}",
      bot.comment.get("id").map_or_else(String::new, jq_string),
      text(&bot.comment, "updatedAt")
    ),
    fixer: (!tick.memory.fixer.is_null()).then_some(&tick.memory.fixer),
  };
  let previous_print = previous
    .and_then(|prev| prev.pointer("/attention/fingerprint"))
    .and_then(Value::as_str);
  let attention = work.attention(previous_print);
  let mut reasons = escalations;
  reasons.extend(facts.signals());
  tick.reasons = reasons;
  tick.decision = decision;
  tick.attention = attention;
}

#[cfg(test)]
#[path = "tests/reduce_test.rs"]
mod tests;
