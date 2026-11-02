//! Immutable delivery, participant-only receipts and bounded inbox reads.

use super::types::{Envelope, Error, Ledger, Message, Session, safe};
use super::{config, encode, membership};
use serde_json::{Value, json};

/// Persist a recipient inbox entry once, rejecting conflicting retries.
pub(super) fn send(
  ledger: &mut Ledger,
  cfg: &config::Config,
  sender: &Session,
  envelope: Envelope,
) -> Result<Value, Error> {
  validate(&envelope)?;
  if envelope.sender != sender.identity() || envelope.group != sender.group {
    return Err(Error::new(
      "unauthorized",
      "sender or collaboration group differs",
    ));
  }
  let key = format!("{}/{}", sender.id, envelope.message_id);
  if let Some(saved) = ledger.messages.get(&key) {
    if saved.envelope != envelope {
      return Err(Error::new(
        "conflict",
        "message ID already has different content",
      ));
    }
    return encode(saved);
  }
  let recipient = recipient(ledger, cfg, &envelope)?;
  conversation(ledger, &envelope)?;
  let wake = if recipient.wake.is_some() {
    "pending"
  } else {
    "unsupported"
  };
  let message = Message {
    key: key.clone(),
    envelope,
    acknowledged: false,
    wake: wake.into(),
  };
  ledger.messages.insert(key, message.clone());
  encode(message)
}

fn recipient<'a>(
  ledger: &'a Ledger,
  cfg: &config::Config,
  envelope: &Envelope,
) -> Result<&'a Session, Error> {
  let recipient = &ledger
    .sessions
    .get(&envelope.recipient.id)
    .ok_or_else(|| Error::new("stale", "recipient is not enrolled"))?
    .session;
  if recipient.group != envelope.group || !membership::registered(cfg, recipient) {
    return Err(Error::new(
      "unauthorized",
      "recipient is outside the authorized group",
    ));
  }
  if recipient.identity() != envelope.recipient || !recipient.active {
    return Err(Error::new(
      "stale",
      "recipient generation is inactive or differs",
    ));
  }
  Ok(recipient)
}

fn validate(envelope: &Envelope) -> Result<(), Error> {
  if envelope.version != 1
    || envelope.timestamp == 0
    || envelope.text.is_empty()
    || envelope.text.len() > 65536
    || ![
      &envelope.sender.id,
      &envelope.sender.generation,
      &envelope.recipient.id,
      &envelope.recipient.generation,
      &envelope.group,
      &envelope.message_id,
      &envelope.conversation_id,
    ]
    .into_iter()
    .all(|value| safe(value))
  {
    return Err(Error::new(
      "invalid",
      "invalid message identity, version, timestamp or text (maximum 64 KiB)",
    ));
  }
  Ok(())
}

fn conversation(ledger: &Ledger, envelope: &Envelope) -> Result<(), Error> {
  let Some(reply_to) = &envelope.reply_to else {
    if envelope.conversation_id != envelope.message_id {
      return Err(Error::new(
        "invalid",
        "a new conversation starts with its message ID",
      ));
    }
    return Ok(());
  };
  let prior = ledger
    .messages
    .get(reply_to)
    .ok_or_else(|| Error::new("invalid", "reply target is absent"))?;
  if prior.envelope.sender != envelope.recipient
    || prior.envelope.recipient != envelope.sender
    || prior.envelope.group != envelope.group
    || prior.envelope.conversation_id != envelope.conversation_id
  {
    return Err(Error::new(
      "invalid",
      "reply participants or conversation differ",
    ));
  }
  Ok(())
}

/// Read at most 32 entries without acknowledging them; the cursor is exclusive.
pub(super) fn inbox(ledger: &Ledger, session: &Session, request: &Value) -> Result<Value, Error> {
  let after = optional_string(request, "after")?;
  let unread_only = match request.get("unread") {
    None => false,
    Some(Value::Bool(value)) => *value,
    _ => return Err(Error::new("invalid", "unread must be boolean")),
  };
  let ours = |message: &&Message| message.envelope.recipient == session.identity();
  let unread = ledger
    .messages
    .values()
    .filter(ours)
    .filter(|m| !m.acknowledged)
    .count();
  let (page, next) = page(ledger.messages.values().filter(ours).filter(|m| {
    (!unread_only || !m.acknowledged) && after.is_none_or(|after| m.key.as_str() > after)
  }))?;
  Ok(json!({"messages":page,"next":next,"unread":unread}))
}

fn page<'a>(
  messages: impl Iterator<Item = &'a Message>,
) -> Result<(Vec<&'a Message>, Option<&'a str>), Error> {
  let mut page = Vec::new();
  let mut bytes = 0;
  let mut more = false;
  for message in messages {
    bytes += serde_json::to_vec(message)
      .map_err(|e| Error::storage(e.to_string()))?
      .len();
    if page.len() == 32 || bytes > 4 * 1024 * 1024 {
      more = true;
      break;
    }
    page.push(message);
  }
  let next = if more {
    page.last().map(|message| message.key.as_str())
  } else {
    None
  };
  Ok((page, next))
}

fn optional_string<'a>(value: &'a Value, name: &str) -> Result<Option<&'a str>, Error> {
  match value.get(name) {
    None | Some(Value::Null) => Ok(None),
    Some(Value::String(value)) if value.len() <= 257 => Ok(Some(value)),
    _ => Err(Error::new("invalid", "invalid inbox cursor")),
  }
}

/// Only participants inspect receipts; only the exact recipient may acknowledge.
pub(super) fn receipt(
  ledger: &mut Ledger,
  session: &Session,
  key: &str,
  ack: bool,
) -> Result<Value, Error> {
  let message = ledger
    .messages
    .get_mut(key)
    .ok_or_else(|| Error::new("invalid", "message is absent"))?;
  let recipient = message.envelope.recipient == session.identity();
  if message.envelope.group != session.group
    || (!recipient && (ack || message.envelope.sender != session.identity()))
  {
    return Err(Error::new(
      "unauthorized",
      "message belongs to other participants",
    ));
  }
  if ack {
    message.acknowledged = true;
  }
  encode(message)
}

#[cfg(test)]
#[path = "tests/messages_test.rs"]
mod tests;
