//! The attention queue: raise with dedup, close on report, acknowledge,
//! answer, reminders, and the rebuild from the journal.

use serde_json::{Value, json};
use toolu_state::time::{epoch_millis, parse_iso};

use crate::journal::Record;
use crate::model::{Ack, Attention, Step, World};
use crate::note::bounded_note;

/// Reminders after the first come at most this often.
const REPEAT_MS: u64 = 60 * 60 * 1000;

/// Journal names that change an item rather than raise one.
const CONTROL: [&str; 5] = ["acknowledged", "answered", "reminder", "reopened", "closed"];

/// Raise `kind` for `key`, or refresh the note of its live item of that kind.
/// Returns the item token.
pub(crate) fn raise(world: &mut World, kind: &str, key: &str, note: &str) -> String {
  let note = bounded_note(note);
  if let Some(item) = world
    .attention
    .iter_mut()
    .find(|item| item.key == key && item.kind == kind)
  {
    if item.note != note {
      item.note.clone_from(&note);
      let record = Record::new("judgment", kind, key, &item.token, &note);
      world.outbox.push_back(Step::Journal(record));
    }
    return item.token.clone();
  }
  world.seq = world.seq.saturating_add(1);
  let token = format!("a{}-{}", world.now_ms, world.seq);
  let record = Record::new("judgment", kind, key, &token, &note);
  let epic = epic_of(world, key);
  world
    .attention
    .push(item(world.seq, &record, epic, world.now_ms));
  world.outbox.push_back(Step::Journal(record));
  world.outbox.push_back(Step::Notify {
    token: token.clone(),
    reminder: false,
  });
  token
}

/// Raise `kind` for `key` as a new event (a stall past its full clock, a new
/// blocked spell): an item a waiter already saw, or one acknowledged or
/// answered, reopens and is announced again. Returns the item token.
pub(crate) fn reraise(world: &mut World, kind: &str, key: &str, note: &str) -> String {
  let now = world.now_ms;
  let Some(item) = world.attention.iter_mut().find(|item| {
    item.key == key && item.kind == kind && (item.delivered || item.state != Ack::Open)
  }) else {
    return raise(world, kind, key, note);
  };
  reopen(item, &bounded_note(note), now);
  let record = Record::new("judgment", "reopened", key, &item.token, &item.note);
  let token = item.token.clone();
  world.outbox.push_back(Step::Journal(record));
  world.outbox.push_back(Step::Notify {
    token: token.clone(),
    reminder: false,
  });
  token
}

fn reopen(item: &mut Attention, note: &str, at: u64) {
  item.state = Ack::Open;
  item.delivered = false;
  note.clone_into(&mut item.note);
  item.announced_ms = at;
  item.reminders = 0;
}

fn item(seq: u64, record: &Record, epic: String, raised_ms: u64) -> Attention {
  Attention {
    seq,
    kind: record.name.clone(),
    key: record.key.clone(),
    epic,
    note: record.note.clone(),
    delivered: false,
    token: record.token.clone(),
    raised_ms,
    state: Ack::Open,
    announced_ms: raised_ms,
    reminders: 0,
    pending: None,
  }
}

/// The epic of an issue key, or the key itself when it names an epic.
fn epic_of(world: &World, key: &str) -> String {
  match world.issues.get(key) {
    Some(issue) => issue.epic.clone(),
    None if world.epics.contains_key(key) => key.to_owned(),
    None => String::new(),
  }
}

/// Close every live item of `key`, journaling its kind and wait.
pub(crate) fn close_key(world: &mut World, key: &str) {
  close_where(world, key, None, "report");
}

/// Close `key`'s live items of `kind` (every kind when `None`) once the engine
/// saw their cause settle (`by`: the merge, a release, a cleanup, a refresh).
pub(crate) fn settle(world: &mut World, key: &str, kind: Option<&str>, by: &str) {
  close_where(world, key, kind, by);
}

fn close_where(world: &mut World, key: &str, kind: Option<&str>, by: &str) {
  if key.is_empty() {
    return;
  }
  let (closed, kept): (Vec<Attention>, Vec<Attention>) = std::mem::take(&mut world.attention)
    .into_iter()
    .partition(|item| item.key == key && kind.is_none_or(|kind| item.kind == kind));
  world.attention = kept;
  for item in closed {
    let waited = world.now_ms.saturating_sub(item.raised_ms);
    let note = format!("kind={} waited_ms={waited} by={by}", item.kind);
    let record = Record::new("judgment", "closed", key, &item.token, &note);
    world.outbox.push_back(Step::Journal(record));
  }
}

fn targets(item: &Attention, key: &str) -> bool {
  if item.key.is_empty() {
    item.kind == key
  } else {
    item.key == key
  }
}

/// Acknowledge every live item of `key`, or a keyless item whose kind is `key`.
pub(crate) fn ack(world: &mut World, key: &str, by: &str) {
  for item in world.attention.iter_mut().filter(|item| targets(item, key)) {
    item.delivered = true;
    if item.state == Ack::Open {
      item.state = Ack::Acknowledged;
      let note = format!("by={by}");
      let record = Record::new("judgment", "acknowledged", &item.key, &item.token, &note);
      world.outbox.push_back(Step::Journal(record));
    }
  }
}

/// Whether `key` has a live item an answer can settle.
pub(crate) fn has_live(world: &World, key: &str) -> bool {
  !key.is_empty() && world.attention.iter().any(|item| item.key == key)
}

/// Journal an answer the worker received and mark the key's live items answered.
pub(crate) fn answered(world: &mut World, key: &str, by: &str, text: &str) {
  let mut first = true;
  for item in world.attention.iter_mut().filter(|item| item.key == key) {
    if first {
      let record = Record::new("prompt", "answer", key, &item.token, text);
      world.outbox.push_back(Step::Journal(record));
      first = false;
    }
    item.delivered = true;
    item.state = Ack::Answered;
    let note = format!("by={by}");
    let record = Record::new("judgment", "answered", key, &item.token, &note);
    world.outbox.push_back(Step::Journal(record));
  }
}

fn due_at(item: &Attention, first_ms: u64) -> Option<u64> {
  let wait = if item.reminders == 0 {
    first_ms
  } else {
    REPEAT_MS
  };
  (item.state == Ack::Open).then(|| item.announced_ms.saturating_add(wait))
}

/// Announce again every open item whose reminder is due.
pub(crate) fn remind(world: &mut World) {
  let (now, first) = (world.now_ms, world.remind_ms);
  for item in &mut world.attention {
    if due_at(item, first).is_none_or(|due| now < due) {
      continue;
    }
    item.reminders = item.reminders.saturating_add(1);
    item.announced_ms = now;
    let note = format!("kind={} reminders={}", item.kind, item.reminders);
    let record = Record::new("judgment", "reminder", &item.key, &item.token, &note);
    world.outbox.push_back(Step::Journal(record));
    world.outbox.push_back(Step::Notify {
      token: item.token.clone(),
      reminder: true,
    });
  }
}

/// Absolute time of the soonest reminder.
pub(crate) fn next_reminder_ms(world: &World) -> Option<u64> {
  world
    .attention
    .iter()
    .filter_map(|item| due_at(item, world.remind_ms))
    .min()
}

/// The oldest item no waiter has seen, now marked seen. A `scope` takes only
/// that epic's items and the engine-wide ones (no epic).
pub(crate) fn take(world: &mut World, scope: Option<&str>) -> Option<Attention> {
  let item = world.attention.iter_mut().find(|item| {
    !item.delivered && scope.is_none_or(|epic| item.epic.is_empty() || item.epic == epic)
  })?;
  item.delivered = true;
  Some(item.clone())
}

/// Keep `lines` as the `wait`-only screen text of the item `token`.
pub(crate) fn set_pending(world: &mut World, token: &str, lines: String) {
  if let Some(item) = world.attention.iter_mut().find(|item| item.token == token) {
    item.pending = (!lines.is_empty()).then_some(lines);
  }
}

/// The public fields of one item, without its screen text.
pub(crate) fn document(item: &Attention, now_ms: u64) -> Value {
  json!({
    "kind": item.kind,
    "key": item.key,
    "epic": item.epic,
    "note": item.note,
    "seq": item.seq,
    "state": item.state.name(),
    "age_ms": now_ms.saturating_sub(item.raised_ms),
  })
}

/// Replay one journal record into the queue while rebuilding. Records
/// without an item token predate the queue and are ignored; a `wait-reply`
/// with one marks that item delivered.
pub(crate) fn observe(world: &mut World, record: &Record) {
  if !record.token.starts_with('a') {
    return;
  }
  if record.kind == "action" && record.name == "wait-reply" {
    if let Some(item) = world
      .attention
      .iter_mut()
      .find(|item| item.token == record.token)
    {
      item.delivered = true;
    }
    return;
  }
  if record.kind != "judgment" {
    return;
  }
  if record.name == "closed" {
    world.attention.retain(|item| item.token != record.token);
    return;
  }
  let at = parse_iso(&record.at).map_or(world.now_ms, epoch_millis);
  if let Some(item) = world
    .attention
    .iter_mut()
    .find(|item| item.token == record.token)
  {
    replay(item, record, at);
    return;
  }
  if CONTROL.contains(&record.name.as_str()) {
    return;
  }
  let seq = record
    .token
    .rsplit_once('-')
    .and_then(|(_, seq)| seq.parse().ok())
    .unwrap_or(record.seq);
  let epic = epic_of(world, &record.key);
  world.attention.push(item(seq, record, epic, at));
}

fn replay(item: &mut Attention, record: &Record, at: u64) {
  match record.name.as_str() {
    "acknowledged" => {
      item.delivered = true;
      item.state = Ack::Acknowledged;
    }
    "answered" => {
      item.delivered = true;
      item.state = Ack::Answered;
    }
    "reminder" => {
      item.reminders = item.reminders.saturating_add(1);
      item.announced_ms = at;
    }
    "reopened" => reopen(item, &record.note, at),
    _ => record.note.clone_into(&mut item.note),
  }
}

#[cfg(test)]
#[path = "tests/attention_test.rs"]
mod tests;
