//! The `ack` and `answer` socket operations on the attention queue.

use std::sync::mpsc::Sender;

use ring::digest::{SHA256, digest};
use serde_json::{Value, json};

use crate::dispatch::result_value;
use crate::journal::{self, Record};
use crate::model::Step;
use crate::schedule::ack;
use crate::server::Engine;

/// Acknowledge the key's attention items for the `ack` op.
pub(crate) fn reply_ack(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  ack(&mut engine.world, key, by);
  engine.flush()?;
  let _sent = reply.send(json!({"ok": true}));
  Ok(false)
}

/// Deliver an answer to a live worker for the `answer` op, then mark the key's
/// attention answered, or record a direction when none is open. A refused or
/// undelivered answer is the caller's error. A journal that fails after the
/// worker received the answer is reported `delivered: uncertain`.
///
/// # Errors
/// The retained journal cannot be read or the admission cannot be written,
/// before any prompt is sent.
pub(crate) fn reply_answer(
  engine: &mut Engine,
  request: &Value,
  reply: &Sender<Value>,
) -> Result<bool, String> {
  let key = request.get("key").and_then(Value::as_str).unwrap_or("");
  let text = request.get("text").and_then(Value::as_str).unwrap_or("");
  let by = request.get("by").and_then(Value::as_str).unwrap_or("user");
  let submission = request
    .get("submission_id")
    .and_then(Value::as_str)
    .filter(|id| !id.is_empty());
  let answer = Answer {
    key,
    text,
    by,
    submission,
  };
  match attempt(engine, &answer)? {
    Attempt::Reply(value) => {
      let _sent = reply.send(value);
      Ok(false)
    }
    Attempt::Maybe(err) => {
      reply_uncertain(engine, &answer, &err, reply);
      Ok(false)
    }
    Attempt::Sent => {
      deliver(engine, &answer, reply);
      Ok(false)
    }
  }
}

/// How far an `answer` got before the worker was reached.
enum Attempt {
  /// Refused, or answered from the journal, with no prompt sent now.
  Reply(Value),
  /// The prompt was delivered.
  Sent,
  /// The prompt may have been delivered; the error says why.
  Maybe(String),
}

/// Validate, dedupe by `submission_id`, then prompt the worker.
fn attempt(engine: &mut Engine, answer: &Answer<'_>) -> Result<Attempt, String> {
  let refuse = |error: String| Ok(Attempt::Reply(result_value(Err(error))));
  if answer.key.is_empty() || answer.text.is_empty() {
    return refuse("answer needs a key and a text".to_owned());
  }
  // Look up an existing admission before the live-agent gate so a retry of a
  // finished id still returns its prior outcome. Journal a new admit only once
  // a live agent exists, or a no-agent refusal would poison later retries.
  if let Some(id) = answer.submission
    && let Some(prior) = recalled(engine, answer, id)?
  {
    return Ok(Attempt::Reply(prior));
  }
  if !crate::attention::has_live_agent(&engine.world, answer.key) {
    return refuse(format!("no live agent for {}", answer.key));
  }
  if let Some(id) = answer.submission {
    record_admit(engine, answer, id)?;
  }
  match send_prompt(engine, answer.key, answer.text) {
    Ok(()) => Ok(Attempt::Sent),
    Err(err) if uncertain(&err) => Ok(Attempt::Maybe(err)),
    Err(err) => {
      refuse_admitted(engine, answer);
      refuse(format!("worker prompt not delivered: {err}"))
    }
  }
}

/// One `answer` request.
struct Answer<'a> {
  key: &'a str,
  text: &'a str,
  by: &'a str,
  submission: Option<&'a str>,
}

impl Answer<'_> {
  /// The submission id as a journal token, empty without one.
  fn token(&self) -> &str {
    self.submission.unwrap_or("")
  }
}

/// Lowercase hex SHA-256 of `key`, a NUL, then `text`.
fn fingerprint(key: &str, text: &str) -> String {
  let mut bytes = Vec::with_capacity(key.len() + 1 + text.len());
  bytes.extend_from_slice(key.as_bytes());
  bytes.push(0);
  bytes.extend_from_slice(text.as_bytes());
  digest(&SHA256, &bytes)
    .as_ref()
    .iter()
    .flat_map(|byte| [byte >> 4, byte & 0x0f])
    .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
    .collect()
}

/// The reply to give when `id` was already used, or `None` when it is new.
fn recalled(engine: &Engine, answer: &Answer<'_>, id: &str) -> Result<Option<Value>, String> {
  let fp = fingerprint(answer.key, answer.text);
  Ok(match prior(engine, id)? {
    Some((seen, _)) if seen != fp => Some(result_value(Err(
      "submission_id reuse with different payload".to_owned(),
    ))),
    Some((_, true)) => Some(json!({"ok": true})),
    Some((_, false)) => Some(json!({
      "ok": true,
      "delivered": "uncertain",
      "note": "worker prompt may not have arrived: earlier submission with this id",
    })),
    None => None,
  })
}

/// Journal `answer-admit` and flush it before the prompt, so a crash or retry
/// cannot type the text twice. The state loop is single-threaded, so retries
/// serialize here.
fn record_admit(engine: &mut Engine, answer: &Answer<'_>, id: &str) -> Result<(), String> {
  let fp = fingerprint(answer.key, answer.text);
  let note = format!("fp={fp} by={}", answer.by);
  let record = Record::new("action", "answer-admit", answer.key, id, &note);
  engine.world.outbox.push_back(Step::Journal(record));
  engine.flush()
}

/// The fingerprint journaled for `id` and whether its answer was delivered,
/// from the retained journal; `None` when the id was never admitted or its
/// last attempt was refused (so a retry is a fresh admission). An in-memory
/// refuse (journal write failed after a definite undelivered prompt) also
/// clears the id for this process.
fn prior(engine: &Engine, id: &str) -> Result<Option<(String, bool)>, String> {
  if engine.refused_ids.contains(id) {
    return Ok(None);
  }
  let mut state = None;
  let rows = journal::tail(&engine.paths.journal_dir(), engine.now)?;
  for row in rows
    .iter()
    .filter(|row| row.kind == "action" && row.token == id)
  {
    match row.name.as_str() {
      "answer-admit" => {
        let fp = row.note.strip_prefix("fp=").unwrap_or("");
        state = Some((fp.split(' ').next().unwrap_or("").to_owned(), false));
      }
      "answer" => {
        if let Some((_, delivered)) = state.as_mut() {
          *delivered = true;
        }
      }
      "answer-refused" => state = None,
      _ => {}
    }
  }
  Ok(state)
}

/// Close an admission whose prompt was refused, so a retry of the id prompts.
/// Writes `answer-refused` straight to the journal (same path as a flushed
/// Journal step). When that write fails, remember the id in-memory so a
/// same-id retry is not stuck as false `delivered: uncertain`.
fn refuse_admitted(engine: &mut Engine, answer: &Answer<'_>) {
  let Some(id) = answer.submission else {
    return;
  };
  let note = format!("by={}", answer.by);
  let mut record = Record::new("action", "answer-refused", answer.key, id, &note);
  if record.epic.is_empty()
    && let Some(issue) = engine.world.issues.get(answer.key)
  {
    record.epic.clone_from(&issue.epic);
  }
  if journal::append(
    &engine.paths.journal_dir(),
    &engine.paths.journal_lock(),
    &record,
    engine.now,
  )
  .is_ok()
  {
    engine.refused_ids.remove(id);
  } else {
    engine.refused_ids.insert(id.to_owned());
  }
}

/// Journal an answer the worker received and mark attention. A journal
/// failure after the worker received it is uncertain, not an error, and
/// restores the attention state so the item stays open.
fn deliver(engine: &mut Engine, answer: &Answer<'_>, reply: &Sender<Value>) {
  let (key, text, by) = (answer.key, answer.text, answer.by);
  let before = (
    engine.world.attention.clone(),
    engine
      .world
      .issues
      .get(key)
      .map(|issue| issue.direction.clone()),
  );
  if crate::attention::has_live(&engine.world, key) {
    crate::attention::answered(&mut engine.world, key, by, text);
  } else {
    crate::attention::directed(&mut engine.world, key, by, text);
  }
  if let Some(id) = answer.submission {
    let record = Record::new("action", "answer", key, id, &format!("by={by}"));
    engine.world.outbox.push_back(Step::Journal(record));
  }
  if let Err(err) = engine.flush() {
    // Rows still queued would be journaled twice by a later flush.
    engine.world.outbox.clear();
    engine.world.attention = before.0;
    if let (Some(issue), Some(direction)) = (engine.world.issues.get_mut(key), before.1) {
      issue.direction = direction;
    }
    reply_uncertain(engine, answer, &format!("journal: {err}"), reply);
    return;
  }
  let _sent = reply.send(result_value(Ok(())));
}

/// Whether a failed prompt may still have reached the worker, by the rules
/// `herdr_calls` marks `uncertain`: herdr's `timeout` and
/// `agent_prompt_stalled` codes (as `result_of` formats them), a transport
/// deadline, a scripted `uncertain` outcome, or a `read:` connection reset
/// after a possibly completed write (macOS `os error 54`, Linux `os error
/// 104`). A bare or `connect:`/`write:` reset is a definite failure, not
/// uncertain. An EPIPE (`broken pipe` / `os error 32`) is also a definite
/// write rejection: `attempt` records `answer-refused` so the same submission
/// id may prompt again.
pub(crate) fn uncertain(err: &str) -> bool {
  let lower = err.to_ascii_lowercase();
  crate::herdr_calls::timed_out(err)
    || [
      ": timeout:",
      ": agent_prompt_stalled:",
      "prompt uncertain",
      "read: connection reset",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// An answer that may have arrived: journal it and leave attention open, so
/// the operator still sees the item. A journal that cannot be written is said
/// so in the note; the reply stays uncertain.
fn reply_uncertain(engine: &mut Engine, answer: &Answer<'_>, err: &str, reply: &Sender<Value>) {
  let (key, text, by) = (answer.key, answer.text, answer.by);
  // The text may have been typed into the agent, so it is journaled like a sent answer.
  let sent = Record::new("prompt", "answer-uncertain", key, answer.token(), text);
  engine.world.outbox.push_back(Step::Journal(sent));
  let record = Record::new(
    "action",
    "answer-uncertain",
    key,
    answer.token(),
    &format!("by={by}"),
  );
  engine.world.outbox.push_back(Step::Journal(record));
  let mut note = format!("worker prompt may not have arrived: {err}");
  if let Err(flush) = engine.flush() {
    engine.world.outbox.clear();
    note = format!("{note}; journal not written: {flush}");
  }
  let _sent = reply.send(json!({"ok": true, "delivered": "uncertain", "note": note}));
}

/// Send `text` to `key`'s worker through herdr, or the `prompt` script,
/// which reads `{"key","text"}`.
///
/// # Errors
/// herdr fails, or the script fails or is deferred.
pub(crate) fn send_prompt(engine: &Engine, key: &str, text: &str) -> Result<(), String> {
  if engine.scripts.is_none() && crate::source::prompt_enabled() {
    let target = engine
      .world
      .issues
      .get(key)
      .map_or_else(|| key.to_owned(), |issue| issue.agent.clone());
    return crate::source::prompt(&engine.paths, &engine.env, &target, text);
  }
  let body = json!({"key": key, "text": text}).to_string();
  let outcome = crate::effects::invoke(
    engine.scripts.as_deref(),
    crate::model::Action::Prompt,
    &body,
  );
  match outcome.as_str() {
    outcome @ ("failed" | "deferred" | "uncertain") => Err(format!("prompt {outcome}")),
    _ => Ok(()),
  }
}

#[cfg(test)]
#[path = "tests/attention_ops_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/attention_redirect_test.rs"]
mod redirect_tests;

#[cfg(test)]
#[path = "tests/attention_submission_test.rs"]
mod submission_tests;
