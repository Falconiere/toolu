//! The notifier thread: a herdr notification and a POST to the secret notify
//! URL for each notice, journaled, so the state loop never waits on either.

use std::sync::mpsc::{Receiver, SyncSender, TrySendError, sync_channel};
use std::time::{Duration, SystemTime};

use serde_json::Value;
use toolu_http::{Auth, Client, Config, Error, Method, Request, check_url};
use toolu_runtime::config::secrets::{self, Secrets};
use toolu_runtime::env::Env;
use toolu_runtime::host::roots::Roots;

use crate::herdr::{self, Call};
use crate::journal::{self, Record};
use crate::model::Step;
use crate::notify_notice::{Notice, build};
use crate::paths::Paths;
use crate::server::Engine;

/// Notices waiting for the thread; more are dropped.
const QUEUE: usize = 64;
/// POST attempts per notice.
const ATTEMPTS: u32 = 5;

/// The first reminder delay and the notifier, when notifications are on.
pub(crate) struct AttentionOptions {
  pub(crate) remind_ms: u64,
  pub(crate) notify: Option<Settings>,
}

impl Default for AttentionOptions {
  /// Reminders after 30 minutes and no notifier.
  fn default() -> Self {
    Self {
      remind_ms: crate::model::REMIND_MS,
      notify: None,
    }
  }
}

/// How the notifier thread delivers.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
  /// Show a herdr notification.
  pub herdr: bool,
  /// POST to the notify URL, when the secret is set.
  pub post: bool,
  /// Environment for secrets, herdr and proxies.
  pub env: Env,
  /// The status dashboard URL, when the listener runs.
  pub dashboard: Option<String>,
  /// Wait before the second attempt; it doubles after each attempt.
  pub backoff: Duration,
  /// Whole-request deadline of one attempt.
  pub timeout: Duration,
  /// DER root certificate for loopback tests.
  pub test_root_ca_der: Option<Vec<u8>>,
}

impl Settings {
  /// Production delays: 10 s per attempt, 1 s doubling between attempts.
  pub(crate) fn new(env: Env, herdr: bool, post: bool) -> Self {
    Self {
      herdr,
      post,
      env,
      dashboard: None,
      backoff: Duration::from_secs(1),
      timeout: Duration::from_secs(10),
      test_root_ca_der: None,
    }
  }
}

/// The engine's handle on the notifier thread.
#[derive(Debug)]
pub(crate) struct Notifier {
  tx: SyncSender<Notice>,
  dashboard: Option<String>,
}

impl Notifier {
  /// Start the thread that delivers notices for `paths`' journal.
  pub(crate) fn spawn(settings: Settings, paths: Paths) -> Self {
    let (tx, rx) = sync_channel(QUEUE);
    let dashboard = settings.dashboard.clone();
    let _thread = std::thread::spawn(move || run(&rx, &settings, &paths));
    Self { tx, dashboard }
  }

  /// Point notices at the status listener bound to `addr`.
  pub(crate) fn set_dashboard(&mut self, addr: std::net::SocketAddr) {
    self.dashboard = Some(crate::notify_notice::dashboard(addr));
  }
}

/// Hand item `token` to the notifier. A full or closed queue drops the notice
/// and journals `notify-dropped`.
pub(crate) fn announce(engine: &mut Engine, token: &str, reminder: bool) {
  let Some(notifier) = &engine.notifier else {
    return;
  };
  let Some(notice) = build(
    &engine.world,
    token,
    reminder,
    notifier.dashboard.as_deref(),
  ) else {
    return;
  };
  let status = match notifier.tx.try_send(notice) {
    Ok(()) => return,
    Err(TrySendError::Full(_)) => "full",
    Err(TrySendError::Disconnected(_)) => "closed",
  };
  let key = engine
    .world
    .attention
    .iter()
    .find(|item| item.token == token)
    .map(|item| item.key.clone())
    .unwrap_or_default();
  let note = format!("channel=queue attempts=0 status={status}");
  let record = Record::new("action", "notify-dropped", &key, token, &note);
  engine.world.outbox.push_back(Step::Journal(record));
}

fn run(rx: &Receiver<Notice>, settings: &Settings, paths: &Paths) {
  let mut client = herdr::Client::new();
  for notice in rx {
    let loaded = secrets::load(&Roots::new(settings.env.clone(), None));
    let none = Secrets::none();
    if settings.herdr {
      show(
        &mut client,
        settings,
        paths,
        &notice,
        loaded.as_ref().unwrap_or(&none),
      );
    }
    if !settings.post {
      continue;
    }
    match &loaded {
      Ok(secrets) => post(settings, paths, &notice, secrets),
      Err(_) => record(
        paths,
        &notice,
        false,
        "channel=url attempts=0 status=secrets",
      ),
    }
  }
}

fn show(
  client: &mut herdr::Client,
  settings: &Settings,
  paths: &Paths,
  notice: &Notice,
  secrets: &Secrets,
) {
  let call = Call::Notify {
    title: secrets::redact_text(&notice.title, secrets),
    body: secrets::redact_text(&notice.body, secrets),
  };
  let result = client.execute(paths, &settings.env, &call);
  let refused = result
    .as_ref()
    .ok()
    .and_then(|body| serde_json::from_str::<Value>(body).ok())
    .is_some_and(|reply| reply.get("error").is_some());
  let status = match (&result, refused) {
    (Ok(_), false) => "ok",
    (Ok(_), true) => "refused",
    (Err(_), _) => "transport",
  };
  let note = format!("channel=herdr attempts=1 status={status}");
  record(paths, notice, status == "ok", &note);
}

/// POST the notice when the notify URL is set, and journal the outcome.
fn post(settings: &Settings, paths: &Paths, notice: &Notice, secrets: &Secrets) {
  let Some(url) = secrets.notify_url() else {
    return;
  };
  let (attempts, status, ok) = deliver(settings, url, notice, secrets);
  let note = format!("channel=url attempts={attempts} status={status}");
  record(paths, notice, ok, &note);
}

/// Attempts made, the last status class, and success.
fn deliver(
  settings: &Settings,
  url: &str,
  notice: &Notice,
  secrets: &Secrets,
) -> (u32, String, bool) {
  let config = Config {
    timeout: settings.timeout,
    max_body_bytes: 64 * 1024,
    max_redirects: 0,
    test_root_ca_der: settings.test_root_ca_der.clone(),
  };
  let (Ok(()), Ok(client)) = (check_url(url), Client::new(config, &settings.env)) else {
    return (0, "refused".to_owned(), false);
  };
  let body = secrets::redact_json(&notice.payload, secrets).to_string();
  let mut last = String::new();
  let mut wait = settings.backoff;
  for attempt in 1..=ATTEMPTS {
    let (status, retry) = classify(&client.send(&Request {
      method: Method::Post,
      url,
      auth: &Auth::None,
      headers: &[("content-type", "application/json")],
      body: Some(body.as_bytes()),
    }));
    if !retry {
      return (
        attempt,
        status.clone(),
        status.starts_with('2') || status.starts_with('3'),
      );
    }
    last = status;
    if attempt < ATTEMPTS {
      std::thread::sleep(wait);
      wait = wait.saturating_mul(2);
    }
  }
  (ATTEMPTS, last, false)
}

/// The status class, and whether another attempt may succeed.
fn classify(result: &Result<toolu_http::Response, Error>) -> (String, bool) {
  match result {
    Ok(response) => {
      let code = response.status;
      (code.to_string(), code == 429 || code >= 500)
    }
    Err(Error::Timeout) => ("timeout".to_owned(), true),
    Err(_) => ("transport".to_owned(), true),
  }
}

fn record(paths: &Paths, notice: &Notice, ok: bool, note: &str) {
  let name = if ok { "notify" } else { "notify-failed" };
  let row = Record::new("action", name, &notice.key, &notice.token, note);
  let _stored = journal::append(
    &paths.journal_dir(),
    &paths.journal_lock(),
    &row,
    SystemTime::now(),
  );
}

#[cfg(test)]
#[path = "tests/notify_test.rs"]
mod tests;

#[cfg(test)]
#[path = "tests/notify_post_contract_test.rs"]
mod post_contract_tests;
