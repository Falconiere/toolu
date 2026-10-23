use std::time::SystemTime;

use super::{ack, answered, has_live, next_reminder_ms, raise, set_pending, take};
use crate::journal::{self, Record};
use crate::logic::{apply_report, ensure_issue};
use crate::model::{Ack, Report, Step, World};
use crate::paths::Paths;

const MINUTE: u64 = 60 * 1000;

fn report(key: &str, phase: &str, note: &str) -> Report {
  Report {
    key: key.to_owned(),
    epic: "one".to_owned(),
    state_dir: "/epic".to_owned(),
    phase: phase.to_owned(),
    pr: None,
    note: note.to_owned(),
  }
}

fn journals(steps: &[Step], name: &str) -> Vec<Record> {
  steps
    .iter()
    .filter_map(journal_of)
    .filter(|record| record.name == name)
    .cloned()
    .collect()
}

fn journal_of(step: &Step) -> Option<&Record> {
  if let Step::Journal(record) = step {
    Some(record)
  } else {
    None
  }
}

fn notices(world: &World) -> Vec<(String, bool)> {
  world
    .outbox
    .iter()
    .filter_map(|step| {
      if let Step::Notify { token, reminder } = step {
        Some((token.clone(), *reminder))
      } else {
        None
      }
    })
    .collect()
}

#[test]
fn a_raise_journals_its_token_and_a_repeat_only_updates_the_note() {
  let mut world = World::new(5_000);
  ensure_issue(&mut world, "a", "one", "/epic");
  let token = raise(&mut world, "babysit", "a", "checks failing");
  assert!(token.starts_with("a5000-"), "{token}");
  assert_eq!(world.attention.len(), 1);
  assert_eq!(world.attention[0].epic, "one");
  assert_eq!(notices(&world), [(token.clone(), false)]);
  world.outbox.clear();
  assert_eq!(raise(&mut world, "babysit", "a", "checks failing"), token);
  assert!(world.outbox.is_empty());
  assert_eq!(raise(&mut world, "babysit", "a", "still failing"), token);
  assert_eq!(world.attention.len(), 1);
  assert_eq!(world.attention[0].note, "still failing");
  assert_eq!(notices(&world), []);
  let steps: Vec<Step> = world.outbox.drain(..).collect();
  assert_eq!(journals(&steps, "babysit")[0].token, token);
  let other = raise(&mut world, "pr-closed", "a", "closed");
  assert_ne!(other, token);
  assert_eq!(world.attention.len(), 2);
}

#[test]
fn reminder_fires_at_the_delay_then_hourly_and_never_when_acknowledged() {
  let mut world = World::new(0);
  let token = raise(&mut world, "needs-human", "a", "pick one");
  let quiet = raise(&mut world, "needs-human", "b", "pick two");
  ack(&mut world, "b", "user");
  world.outbox.clear();
  assert_eq!(next_reminder_ms(&world), Some(30 * MINUTE));
  let mut fired = Vec::new();
  for minute in [29, 30, 31, 89, 90] {
    world.now_ms = minute * MINUTE;
    crate::schedule::on_tick(&mut world);
    for (notice, reminder) in notices(&world) {
      assert!(reminder);
      fired.push((minute, notice));
    }
    world.outbox.clear();
  }
  assert_eq!(fired, [(30, token.clone()), (90, token)]);
  assert!(!fired.iter().any(|(_, notice)| notice == &quiet));
  assert_eq!(world.attention.len(), 2);
  assert_eq!(world.attention[0].reminders, 2);
  assert_eq!(next_reminder_ms(&world), Some(150 * MINUTE));
  assert_eq!(
    take(&mut world, None).map(|item| item.key),
    Some("a".to_owned())
  );
  assert!(take(&mut world, None).is_none());
}

#[test]
fn reminder_records_name_the_kind_and_count() {
  let mut world = World::new(0);
  world.remind_ms = MINUTE;
  let token = raise(&mut world, "failed", "a", "boom");
  world.now_ms = MINUTE;
  crate::attention::remind(&mut world);
  let steps: Vec<Step> = world.outbox.drain(..).collect();
  let reminders = journals(&steps, "reminder");
  assert_eq!(reminders.len(), 1);
  assert_eq!(reminders[0].token, token);
  assert_eq!(reminders[0].note, "kind=failed reminders=1");
}

#[test]
fn closed_records_link_the_raise_by_token_with_kind_and_wait() {
  let mut world = World::new(1_000);
  ensure_issue(&mut world, "a", "one", "/epic");
  let raised = apply_report(&mut world, &report("a", "needs-human", "which?"));
  let raise_record = journals(&raised, "needs-human").remove(0);
  world.now_ms = 61_000;
  let steps = apply_report(&mut world, &report("a", "needs-human", "and now?"));
  let closed = journals(&steps, "closed");
  assert_eq!(closed.len(), 1);
  assert_eq!(closed[0].token, raise_record.token);
  assert_eq!(closed[0].note, "kind=needs-human waited_ms=60000 by=report");
  let reopened = journals(&steps, "needs-human");
  assert_eq!(reopened.len(), 1);
  assert_ne!(reopened[0].token, raise_record.token);
  let order: Vec<&str> = steps
    .iter()
    .filter_map(journal_of)
    .filter(|record| record.kind == "judgment")
    .map(|record| record.name.as_str())
    .collect();
  assert_eq!(order, ["closed", "needs-human"]);
  let done = apply_report(&mut world, &report("a", "execution", ""));
  assert_eq!(journals(&done, "closed").len(), 1);
  assert_eq!(world.attention.len(), 0);
}

#[test]
fn closed_records_cover_a_stall_raised_by_a_tick() {
  let mut world = World::new(0);
  world.stall_ms = 10;
  ensure_issue(&mut world, "a", "one", "/epic").phase = "execution".to_owned();
  world.issues.get_mut("a").expect("issue").phase_at_ms = 0;
  for now in [10, 20] {
    world.now_ms = now;
    crate::schedule::on_tick(&mut world);
  }
  let token = world.attention[0].token.clone();
  assert_eq!(world.attention[0].kind, "stall");
  world.outbox.clear();
  world.now_ms = 50;
  let steps = apply_report(&mut world, &report("a", "plan", ""));
  let closed = journals(&steps, "closed");
  assert_eq!(closed[0].token, token);
  assert_eq!(closed[0].note, "kind=stall waited_ms=30 by=report");
}

#[test]
fn ack_by_key_or_by_kind_for_a_keyless_item() {
  let mut world = World::new(0);
  raise(&mut world, "dependency-cycle", "", "cycle");
  raise(&mut world, "stall", "a", "stall");
  world.outbox.clear();
  ack(&mut world, "nobody", "user");
  assert!(world.outbox.is_empty());
  ack(&mut world, "dependency-cycle", "codex");
  ack(&mut world, "a", "user");
  assert!(
    world
      .attention
      .iter()
      .all(|item| item.state == Ack::Acknowledged)
  );
  assert!(world.attention.iter().all(|item| item.delivered));
  let steps: Vec<Step> = world.outbox.drain(..).collect();
  let acks = journals(&steps, "acknowledged");
  assert_eq!(acks.len(), 2);
  assert_eq!(acks[0].note, "by=codex");
  ack(&mut world, "a", "user");
  assert!(world.outbox.is_empty());
  assert_eq!(next_reminder_ms(&world), None);
}

#[test]
fn an_answer_marks_live_items_and_journals_who_answered() {
  let mut world = World::new(0);
  assert!(!has_live(&world, "a"));
  assert!(!has_live(&world, ""));
  let token = raise(&mut world, "needs-human", "a", "which?");
  assert!(has_live(&world, "a"));
  world.outbox.clear();
  answered(&mut world, "a", "user", "take B");
  assert_eq!(world.attention[0].state, Ack::Answered);
  let steps: Vec<Step> = world.outbox.drain(..).collect();
  let prompt = journals(&steps, "answer");
  assert_eq!(prompt[0].kind, "prompt");
  assert_eq!(prompt[0].token, token);
  assert_eq!(prompt[0].note, "take B");
  assert_eq!(journals(&steps, "answered")[0].note, "by=user");
  assert_eq!(next_reminder_ms(&world), None);
}

#[test]
fn pending_lines_stay_on_the_item_and_out_of_its_document() {
  let mut world = World::new(0);
  let token = raise(&mut world, "blocked", "a", "blocked");
  set_pending(&mut world, &token, "Run it? (y/n)".to_owned());
  assert_eq!(world.attention[0].pending.as_deref(), Some("Run it? (y/n)"));
  let doc = super::document(&world.attention[0], 250);
  assert!(doc.get("pending").is_none());
  assert_eq!(doc["state"], "open");
  assert_eq!(doc["age_ms"], 250);
  set_pending(&mut world, &token, String::new());
  assert_eq!(world.attention[0].pending, None);
}

#[test]
fn attention_rebuild_restores_open_and_acknowledged_items_only() {
  let temp = tempfile::tempdir().expect("temp");
  let paths = Paths::at(temp.path());
  let now = SystemTime::now();
  let rows = [
    Record::new("judgment", "needs-human", "a", "a1-1", "which?"),
    Record::new("judgment", "stall", "b", "a2-2", "stalled"),
    Record::new("judgment", "acknowledged", "b", "a2-2", "by=user"),
    Record::new("judgment", "failed", "c", "a3-3", "boom"),
    Record::new(
      "judgment",
      "closed",
      "c",
      "a3-3",
      "kind=failed waited_ms=1 by=report",
    ),
    Record::new("judgment", "needs-human", "d", "", "legacy"),
    Record::new(
      "judgment",
      "reminder",
      "a",
      "a1-1",
      "kind=needs-human reminders=1",
    ),
    Record::new("judgment", "acknowledged", "z", "a9-9", "by=user"),
  ];
  for row in &rows {
    journal::append(&paths.journal_dir(), &paths.journal_lock(), row, now).expect("append");
  }
  let world = crate::snapshot::load(&paths, now).expect("load");
  assert_rebuilt(&world);
}

fn assert_rebuilt(world: &World) {
  let items: Vec<(&str, &str, &str, Ack)> = world
    .attention
    .iter()
    .map(|item| {
      (
        item.key.as_str(),
        item.kind.as_str(),
        item.note.as_str(),
        item.state,
      )
    })
    .collect();
  assert_eq!(
    items,
    [
      ("a", "needs-human", "which?", Ack::Open),
      ("b", "stall", "stalled", Ack::Acknowledged),
    ]
  );
  assert_eq!(world.attention[0].seq, 1);
  assert_eq!(world.attention[0].reminders, 1);
  assert!(!world.attention[0].delivered);
  assert!(
    world
      .outbox
      .iter()
      .all(|step| !matches!(step, Step::Notify { .. }))
  );
}
