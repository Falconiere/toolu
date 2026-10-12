//! What the engine does with one babysit decision for a watched pull
//! request: a green tick queues it for merge, a finding or an escalation
//! prompts the worker and takes a queued issue back to babysit (#448), and
//! a failed tick is an attention item that leaves the queue alone.

use crate::babysit::Next;
use crate::journal::Record;
use crate::logic::push_attention;
use crate::model::{Step, World};

/// Apply `next` to the issue that `watch_key`'s pull request belongs to.
pub(crate) fn apply(world: &mut World, watch_key: &str, next: Next) {
  let Some(crate::watch::Kind::Pr { key, .. }) =
    world.watches.get(watch_key).map(|watch| &watch.kind)
  else {
    return;
  };
  let key = key.clone();
  match next {
    Next::KeepGoing => {}
    Next::MergeQueue => {
      if let Some(issue) = world.issues.get_mut(&key)
        && issue.phase != "ready"
      {
        "ready".clone_into(&mut issue.phase);
        crate::queue_fields::note_phase(issue, "ready", world.now_ms);
        world.outbox.push_back(Step::Status { key: key.clone() });
        world.outbox.push_back(Step::Issue { key: key.clone() });
        world.outbox.push_back(Step::Journal(Record::new(
          "transition",
          "github-babysit-ready",
          &key,
          "",
          "",
        )));
      }
    }
    Next::Attention(note) => push_attention(world, "babysit", &key, &note),
    Next::Prompt(text) => {
      if !hold(world, &key, &text, None) {
        leave_queue(world, &key);
        crate::babysit::prompt(world, &key, "babysit", text, Some("babysit"));
      }
    }
    Next::Escalate { note, prompt: text } => {
      if !hold(world, &key, &text, Some(&note)) {
        leave_queue(world, &key);
        push_attention(world, "babysit", &key, &note);
        crate::babysit::prompt(world, &key, "babysit", text, None);
      }
    }
  }
}

/// A queue step (possibly a rebase in the worktree) is armed or running:
/// hold the prompt until it commits, so the worker never edits under it.
fn hold(world: &mut World, key: &str, prompt: &str, note: Option<&str>) -> bool {
  let Some(issue) = world.issues.get_mut(key) else {
    return false;
  };
  let stepping = issue
    .pending
    .as_ref()
    .is_some_and(|pending| pending.action == crate::model::Action::Merge);
  if stepping {
    issue.queue.held = Some(crate::queue_fields::Held {
      prompt: prompt.to_owned(),
      note: note.map(str::to_owned),
    });
    world.outbox.push_back(crate::model::Step::Issue {
      key: key.to_owned(),
    });
  }
  stepping
}

/// A babysit finding on a queued issue sends it back to babysit.
fn leave_queue(world: &mut World, key: &str) {
  if world.issues.get(key).is_some_and(crate::queue::queued) {
    crate::commit_queue::leave(world, key, "babysit", "babysit needs the worker");
  }
}

#[cfg(test)]
#[path = "tests/babysit_next_test.rs"]
mod tests;
