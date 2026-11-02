//! Group authorization and generation-fenced membership.

use super::types::{Auth, Error, Ledger, Registration, Session, credential, safe};
use super::{config, encode, field};
use serde_json::Value;

/// Enroll once using a shared group credential and a private session capability.
pub(super) fn join(
  ledger: &mut Ledger,
  cfg: &config::Config,
  request: &Value,
) -> Result<Value, Error> {
  let session: Session = field(request, "session")?;
  let secret: String = field(request, "secret")?;
  let group_key: String = field(request, "group_key")?;
  authorize(ledger, cfg, &session, (&secret, &group_key))?;
  if let Some(saved) = ledger.sessions.get(&session.id) {
    if !saved.session.active {
      return Err(Error::new(
        "stale",
        "session has left; join with a fresh identity",
      ));
    }
    if saved.session != session || saved.secret != secret {
      return Err(Error::new(
        "conflict",
        "session identity is already enrolled",
      ));
    }
    return encode(&saved.session);
  }
  ledger.groups.insert(session.group.clone(), group_key);
  ledger.sessions.insert(
    session.id.clone(),
    Registration {
      session: session.clone(),
      secret,
    },
  );
  encode(session)
}

fn authorize(
  ledger: &Ledger,
  cfg: &config::Config,
  session: &Session,
  credentials: (&str, &str),
) -> Result<(), Error> {
  let (secret, group_key) = credentials;
  validate(session)?;
  if !credential(secret) || !credential(group_key) {
    return Err(Error::new(
      "invalid",
      "credentials must be 32..256 printable ASCII bytes",
    ));
  }
  if !registered(cfg, session) {
    return Err(Error::new(
      "unauthorized",
      "machine or coding host is not registered",
    ));
  }
  if ledger
    .groups
    .get(&session.group)
    .is_some_and(|saved| saved != group_key)
  {
    return Err(Error::new("unauthorized", "group credential differs"));
  }
  Ok(())
}

fn validate(session: &Session) -> Result<(), Error> {
  if ![
    &session.id,
    &session.generation,
    &session.group,
    &session.machine,
  ]
  .into_iter()
  .all(|value| safe(value))
    || !session.active
  {
    return Err(Error::new("invalid", "invalid peer session"));
  }
  if let Some(wake) = &session.wake {
    let prefix = format!("{}/t", session.machine);
    let valid = wake.address.strip_prefix(&prefix).is_some_and(|id| {
      !id.is_empty() && id.len() <= 20 && id.bytes().all(|byte| byte.is_ascii_digit())
    });
    if !valid
      || wake.owner != format!("{}:{}", session.id, session.generation)
      || wake.created_ms == 0
      || wake.pid == 0
    {
      return Err(Error::new("invalid", "invalid terminal incarnation"));
    }
  }
  Ok(())
}

/// A registered machine supports this session's coding host, even while draining.
pub(super) fn registered(cfg: &config::Config, session: &Session) -> bool {
  cfg
    .machines
    .get(&session.machine)
    .is_some_and(|machine| machine.agents.contains(&session.host))
}

/// Verify the private capability, exact generation and current machine enrollment.
pub(super) fn authenticate(
  ledger: &Ledger,
  cfg: &config::Config,
  auth: &Auth,
  allow_left: bool,
) -> Result<Session, Error> {
  let saved = ledger
    .sessions
    .get(&auth.id)
    .ok_or_else(|| Error::new("unauthorized", "unknown peer session"))?;
  if saved.secret != auth.secret {
    return Err(Error::new("unauthorized", "session credential differs"));
  }
  if saved.session.generation != auth.generation || (!allow_left && !saved.session.active) {
    return Err(Error::new(
      "stale",
      "session generation is inactive or differs",
    ));
  }
  if !registered(cfg, &saved.session) {
    return Err(Error::new(
      "unauthorized",
      "machine or coding host registration was removed",
    ));
  }
  Ok(saved.session.clone())
}

/// Retire a session without discarding its inbox or receipts.
pub(super) fn leave(ledger: &mut Ledger, session: &Session) -> Result<Value, Error> {
  let entry = ledger
    .sessions
    .get_mut(&session.id)
    .ok_or_else(|| Error::new("unauthorized", "unknown session"))?;
  entry.session.active = false;
  encode(&entry.session)
}

#[cfg(test)]
#[path = "tests/membership_test.rs"]
mod tests;
