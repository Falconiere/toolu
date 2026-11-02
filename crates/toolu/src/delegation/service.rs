//! Controller-independent relay service, run in an owned Herdr terminal.

use super::{Env, Value, client, config, json};
use std::time::{Duration, SystemTime};
use toolu_mux::hc::{Client, Launch, require_owner};

const NAME: &str = "toolu-relay";
const OWNER: &str = "toolu.owner=toolu-relay";

/// Select a drained authority and prepare its owned service.
pub(super) fn select(env: &Env, machine: &str) -> Result<Value, String> {
  super::selection::select(env, machine)
}

/// Start or recover only the owned relay terminal.
pub(super) fn start(env: &Env) -> Result<Value, String> {
  client::call(env, &json!({"op":"authority"}))?;
  let cfg = config::load(&config::root(env))?;
  let machine = cfg.relay.ok_or("relay is not configured")?;
  let target = format!("{machine}/{NAME}");
  let client = Client::new(env);
  match client.get(&target) {
    Ok(terminal) => {
      require_owner(&terminal, OWNER).map_err(|e| e.message)?;
      if terminal.get("state").and_then(Value::as_str) != Some("finished") {
        return Ok(terminal);
      }
      client
        .call(&["rm", &target], Duration::from_secs(30))
        .map_err(|e| e.message)?;
    }
    Err(error) if error.code == "not_found" => (),
    Err(error) => return Err(error.message),
  }
  let argv = vec!["toolu".into(), "relay".into(), "serve".into()];
  client
    .launch(&Launch {
      name: NAME,
      owner: OWNER,
      cwd: "/",
      machine: Some(&machine),
      argv: &argv,
    })
    .map_err(|e| e.message)
}

/// Keep local configuration visible during relay outages.
pub(super) fn status(env: &Env) -> Result<Value, String> {
  let cfg = config::load(&config::root(env))?;
  if cfg.relay.is_none() {
    return Ok(json!({"config":cfg,"tasks":[],"service":null}));
  }
  let runtime =
    client::call(env, &json!({"op":"status"})).unwrap_or_else(|error| json!({"error":error}));
  let relay = cfg.relay.as_deref().ok_or("missing relay")?;
  let terminal = Client::new(env)
    .get(&format!("{relay}/{NAME}"))
    .unwrap_or_else(|error| json!({"error":error.message}));
  Ok(json!({"config":cfg,"runtime":runtime,"service":terminal}))
}

/// Dispatch until this authority has been durably retired.
pub(super) fn serve(env: &Env) -> Result<Value, String> {
  let root = config::root(env);
  if !root.join("authority.json").is_file() {
    return Err("this machine is not the designated relay; run toolu setup relay first".into());
  }
  let options = toolu_state::lock::LockOptions {
    timeout: Duration::from_secs(5),
    stale: Duration::MAX,
  };
  toolu_state::lock::with_lock_strict(&root.join("service"), options, || service_loop(env, &root))?
}

fn service_loop(env: &Env, root: &std::path::Path) -> Result<Value, String> {
  let peer_root = root.to_path_buf();
  let peer_env = env.clone();
  let _peer_notices = std::thread::spawn(move || {
    toolu_mux::relay::peers::wake::serve(&peer_root, &peer_env);
  });
  while root.join("authority.json").is_file() {
    toolu_mux::relay::rpc::check_authority(root)?;
    let result = toolu_mux::relay::driver::tick(root, env);
    let at = toolu_state::time::epoch_millis(SystemTime::now());
    let health = match result {
      Ok(tasks) => json!({"at":at,"tasks":tasks.len(),"error":null}),
      Err(error) => json!({"at":at,"error":error}),
    };
    if !toolu_state::io::write_atomic(&root.join("health.json"), &health.to_string()) {
      return Err("cannot save relay health".into());
    }
    std::thread::sleep(Duration::from_secs(1));
  }
  Ok(json!({"retired":true}))
}

#[cfg(test)]
#[path = "tests/service_test.rs"]
mod tests;
