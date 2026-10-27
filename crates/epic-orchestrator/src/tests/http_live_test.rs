use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use toolu_runtime::config::secrets::Secrets;
use toolu_runtime::env::Env;

use super::{Herdr, Limiter, Live, Sources};
use crate::fleet_rig::{config, get, listener, secrets_of};
use crate::herdr_fixture::HerdrFixture;
use crate::model::{Issue, World};

const TOKEN: &str = "LIVE-TITLE-TOKEN-CANARY";

fn world_with(key: &str, kind: &str) -> Mutex<World> {
  let mut world = World::new(0);
  let mut issue = Issue::blank(key, "one", "/tmp/one", 0);
  issue.kind = kind.to_owned();
  world.issues.insert(key.to_owned(), issue);
  Mutex::new(world)
}

fn body(live: &Live, herdr: &Herdr, world: &Mutex<World>, secrets: &Secrets) -> (u16, Value) {
  let limiter = Limiter::default();
  let response = live.response(&Sources {
    herdr,
    limiter: &limiter,
    world,
    secrets,
  });
  let value = serde_json::from_slice(&response.body).expect("json body");
  (response.status, value)
}

fn start_agent(fixture: &HerdrFixture, pane: &str, name: &str) {
  fixture
    .state()
    .start(&json!({"name": name, "kind": "claude", "pane_id": pane}))
    .unwrap_or_else(|_| panic!("start {name}"));
}

fn agent_lists(fixture: &HerdrFixture) -> usize {
  let calls = fixture.calls();
  calls.iter().filter(|(method, _)| method == "agent.list").count()
}

fn set_agent(fixture: &HerdrFixture, name: &str, status: &str, title: &str) {
  let mut state = fixture.state();
  let agent = state.agents.iter_mut().find(|agent| agent.name == name);
  let agent = agent.unwrap_or_else(|| panic!("agent {name}"));
  status.clone_into(&mut agent.status);
  title.clone_into(&mut agent.title);
}

/// A herdr server with a worker agent `a` and an unmapped agent `stray`.
fn two_agents() -> (HerdrFixture, tempfile::TempDir) {
  let fixture = HerdrFixture::start();
  let repo = tempfile::tempdir().expect("repo");
  let pane = format!("{}:p1", fixture.add_repo(repo.path()));
  start_agent(&fixture, &pane, "a");
  start_agent(&fixture, &pane, "stray");
  (fixture, repo)
}

#[test]
fn live_lists_mapped_workers_from_one_shared_sample_with_safe_titles() {
  let (fixture, repo) = two_agents();
  set_agent(&fixture, "a", "working", &format!("{TOKEN} {}", "é".repeat(200)));
  let secrets = secrets_of(repo.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let world = world_with("a", "claude");
  let herdr = Herdr::Socket(fixture.env());
  let live = Live::default();
  let (status, first) = body(&live, &herdr, &world, &secrets);
  let (_, second) = body(&live, &herdr, &world, &secrets);
  assert_eq!(status, 200);
  assert_eq!(first["sampled_at"], second["sampled_at"]);
  assert_eq!(agent_lists(&fixture), 1, "one herdr call per cache window");
  let items = first["items"].as_array().expect("items");
  assert_eq!(items.len(), 1, "an unmapped agent is never listed: {first}");
  let item = &items[0];
  assert_eq!(
    (&item["key"], &item["epic"], &item["host"], &item["agent_status"]),
    (&json!("a"), &json!("one"), &json!("claude"), &json!("working"))
  );
  assert_eq!(item["sampled_at"], first["sampled_at"]);
  let title = item["title"].as_str().expect("title");
  assert!(title.starts_with("<redacted> é") && !title.contains(TOKEN), "{title}");
  assert_eq!(title.chars().count(), 160);
}

#[test]
fn live_reports_a_status_change_once_the_cache_window_ends() {
  let (fixture, _repo) = two_agents();
  let world = world_with("a", "");
  let herdr = Herdr::Socket(fixture.env());
  let live = Live::default();
  let (_, before) = body(&live, &herdr, &world, &Secrets::none());
  assert_eq!(before["items"][0]["agent_status"], "idle");
  assert_eq!(before["items"][0]["host"], Value::Null);
  set_agent(&fixture, "a", "working", "");
  std::thread::sleep(Duration::from_millis(2_100));
  let (_, after) = body(&live, &herdr, &world, &Secrets::none());
  assert_ne!(after["sampled_at"], before["sampled_at"]);
  assert_eq!(after["items"][0]["agent_status"], "working");
  assert_eq!(after["items"][0]["title"], Value::Null);
  assert_eq!(agent_lists(&fixture), 2);
}

#[test]
fn unavailable_herdr_answers_at_once_with_a_reason() {
  let fixture = HerdrFixture::start();
  fixture.state().protocol = 23;
  let absent = tempfile::tempdir().expect("absent");
  let missing = Env::from_pairs([(
    "HERDR_SOCKET_PATH",
    absent.path().join("herdr.sock").display().to_string(),
  )]);
  let world = world_with("a", "");
  for (herdr, reason) in [
    (Herdr::Off, "off"),
    (Herdr::Scripted, "scripted"),
    (Herdr::Socket(missing), "not running"),
    (Herdr::Socket(fixture.env()), "protocol 23"),
  ] {
    let started = Instant::now();
    let (status, value) = body(&Live::default(), &herdr, &world, &Secrets::none());
    assert!(started.elapsed() < Duration::from_secs(2), "{reason}");
    assert_eq!(status, 200);
    assert_eq!(value["available"], false);
    assert_eq!(value["reason"], reason);
    assert_eq!(value["items"], json!([]));
  }
  assert_eq!(agent_lists(&fixture), 0, "no agent.list on a protocol mismatch");
}

#[test]
fn the_limiter_holds_eight_slots_and_frees_one_on_drop() {
  let limiter = Limiter::default();
  let held: Vec<_> = (0..8).filter_map(|_| limiter.try_acquire()).collect();
  assert_eq!(held.len(), 8);
  assert!(limiter.try_acquire().is_none());
  drop(held);
  assert!(limiter.try_acquire().is_some());
}

/// A herdr socket that accepts and never answers: a stalled server.
fn silent_herdr(dir: &std::path::Path) -> (Env, Arc<AtomicUsize>) {
  let path = dir.join("herdr.sock");
  let socket = UnixListener::bind(&path).expect("bind");
  let accepted = Arc::new(AtomicUsize::new(0));
  let count = Arc::clone(&accepted);
  std::thread::spawn(move || {
    let mut open = Vec::new();
    for stream in socket.incoming().map_while(Result::ok) {
      count.fetch_add(1, Ordering::SeqCst);
      open.push(stream);
    }
  });
  let env = Env::from_pairs([("HERDR_SOCKET_PATH", path.display().to_string())]);
  (env, accepted)
}

#[test]
fn overlapping_requests_past_eight_get_herdr_busy_while_overview_answers() {
  let root = tempfile::tempdir().expect("root");
  let (env, accepted) = silent_herdr(root.path());
  let mut http = config(0, Vec::new(), Secrets::none());
  http.herdr = Herdr::Socket(env);
  let (running, stop) = listener(root.path(), http);
  let addr = running.addr;
  let requests: Vec<_> = (0..9)
    .map(|_| std::thread::spawn(move || get(addr, "/api/v1/agents/live")))
    .collect();
  std::thread::sleep(Duration::from_millis(200));
  let started = Instant::now();
  let (overview, _) = get(addr, "/api/v1/overview");
  let took = started.elapsed();
  let answers: Vec<(u16, String)> = requests
    .into_iter()
    .map(|request| request.join().expect("request"))
    .collect();
  assert_eq!(overview, 200);
  assert!(took < Duration::from_millis(500), "overview took {took:?}");
  let busy = answers.iter().filter(|(status, _)| *status == 503).count();
  assert!(busy >= 1, "{answers:?}");
  assert!(answers.iter().all(|(status, body)| *status == 200
    || (*status == 503 && body.contains("herdr_busy"))));
  assert_eq!(accepted.load(Ordering::SeqCst), 1, "one stalled herdr call at a time");
  running.stop(&stop).expect("listener");
}

#[test]
fn live_needs_the_token_off_loopback_and_is_never_proxied() {
  let root = tempfile::tempdir().expect("root");
  let secrets = secrets_of(root.path(), &[("TOOLU_EPIC_STATUS_TOKEN", TOKEN)]);
  let mut public = config(0, Vec::new(), secrets);
  public.addr = "0.0.0.0:0".parse().expect("address");
  let (running, stop) = listener(root.path(), public);
  let addr = std::net::SocketAddr::from(([127, 0, 0, 1], running.addr.port()));
  let (denied, _) = get(addr, "/api/v1/agents/live");
  assert_eq!(denied, 401);
  running.stop(&stop).expect("listener");
  let peer = crate::peer_fixture::peer("box", addr);
  let (running, stop) = listener(
    root.path(),
    config(0, vec![peer], Secrets::none()),
  );
  let (local, value) = get(running.addr, "/api/v1/agents/live");
  let (proxied, _) = get(running.addr, "/api/v1/fleet/peers/box/agents/live");
  assert_eq!(local, 200);
  assert!(value.contains("\"reason\":\"off\""), "{value}");
  assert_eq!(proxied, 404);
  running.stop(&stop).expect("listener");
}
