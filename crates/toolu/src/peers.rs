//! Peer session messaging commands.
use clap::{Arg, ArgMatches, Command};
use serde_json::{Value, json};
use std::time::Duration;

/// Opt-in peer lifecycle registration and bounded unread notices.
pub(crate) mod notice;
use toolu_mux::relay::peers::client;
use toolu_protocol::exit::Exit;
use toolu_runtime::{
  cli::{Ctx, Outcome},
  env::Env,
};

fn flag(name: &'static str) -> Arg {
  Arg::new(name).long(name).required(true)
}
/// Peer messaging command tree.
pub fn command() -> Command {
  Command::new("peers")
    .about("Communicate with peer agent sessions")
    .arg(
      Arg::new("session")
        .long("session")
        .help("Saved peer identity; defaults to TOOLU_PEERS_SESSION"),
    )
    .subcommands([join_command(), send_command()])
    .subcommand(
      Command::new("reply")
        .about("Reply to an incoming message in its conversation")
        .arg(Arg::new("key").required(true))
        .args([flag("text"), Arg::new("id").long("id")]),
    )
    .subcommand(
      Command::new("inbox")
        .about("Read a page of messages without acknowledging them")
        .arg(Arg::new("after").long("after"))
        .arg(
          Arg::new("unread")
            .long("unread")
            .action(clap::ArgAction::SetTrue),
        ),
    )
    .subcommand(
      Command::new("status")
        .about("Recover queued sends and inspect receipts")
        .arg(Arg::new("key")),
    )
    .subcommand(
      Command::new("ack")
        .about("Explicitly acknowledge an incoming message")
        .arg(Arg::new("key").required(true)),
    )
    .subcommands([
      Command::new("list").about("Discover active members of this group"),
      Command::new("leave").about("Retire this session without deleting messages"),
    ])
}

fn join_command() -> Command {
  Command::new("join")
    .about("Enroll a fresh session in an authorized group")
    .args([
      flag("group"),
      flag("machine"),
      flag("agent").value_parser(["claude", "codex", "muse", "cursor", "opencode"]),
      Arg::new("key-file").long("key-file"),
    ])
}

fn send_command() -> Command {
  Command::new("send")
    .about("Persist a message and attempt inbox delivery")
    .args([
      flag("to"),
      flag("generation"),
      flag("text"),
      Arg::new("id").long("id"),
    ])
}
fn value<'a>(m: &'a ArgMatches, key: &str) -> &'a str {
  m.get_one::<String>(key).map_or("", String::as_str)
}
fn execute(m: &ArgMatches, env: &Env) -> Result<Value, String> {
  let (action, args) = m.subcommand().ok_or("peer operation required")?;
  let timeout = Duration::from_secs(130);
  if action == "join" {
    return join(args, env, timeout);
  }
  let id = m
    .get_one::<String>("session")
    .map(String::as_str)
    .or_else(|| env.get("TOOLU_PEERS_SESSION"))
    .ok_or("--session or TOOLU_PEERS_SESSION required")?;
  match action {
    "send" => client::send(
      env,
      id,
      &client::Send {
        recipient: json!({"id":value(args,"to"),"generation":value(args,"generation")}),
        text: value(args, "text"),
        message_id: args.get_one::<String>("id").map(String::as_str),
        reply_to: None,
        conversation_id: None,
      },
    ),
    "reply" => reply(args, env, id, timeout),
    "inbox" => client::request(
      env,
      id,
      action,
      json!({"after":args.get_one::<String>("after"),"unread":args.get_flag("unread")}),
      timeout,
    ),
    "status" => status(args, env, id),
    "ack" => client::request(
      env,
      id,
      action,
      json!({"key":args.get_one::<String>("key")}),
      timeout,
    ),
    "list" | "leave" => client::request(env, id, action, json!({}), timeout),
    _ => Err("unknown peer operation".into()),
  }
}
fn join(args: &ArgMatches, env: &Env, timeout: Duration) -> Result<Value, String> {
  let key = args
    .get_one::<String>("key-file")
    .map(String::as_str)
    .or_else(|| env.get("TOOLU_PEERS_KEY_FILE"))
    .ok_or("--key-file or TOOLU_PEERS_KEY_FILE required")?;
  client::join(
    env,
    &client::Join {
      group: value(args, "group"),
      machine: value(args, "machine"),
      host: value(args, "agent"),
      key_file: key,
    },
    timeout,
  )
}
fn reply(args: &ArgMatches, env: &Env, id: &str, timeout: Duration) -> Result<Value, String> {
  let key = value(args, "key");
  let original = client::request(env, id, "status", json!({"key":key}), timeout)?;
  let envelope = original.get("envelope").ok_or("invalid message envelope")?;
  if envelope.pointer("/recipient/id").and_then(Value::as_str) != Some(id) {
    return Err("only the recipient can reply to this message".into());
  }
  client::send(
    env,
    id,
    &client::Send {
      recipient: envelope
        .get("sender")
        .cloned()
        .ok_or("invalid message sender")?,
      text: value(args, "text"),
      message_id: args.get_one::<String>("id").map(String::as_str),
      reply_to: Some(key),
      conversation_id: envelope.get("conversation_id").and_then(Value::as_str),
    },
  )
}
fn status(args: &ArgMatches, env: &Env, id: &str) -> Result<Value, String> {
  let timeout = Duration::from_secs(2);
  let key = args.get_one::<String>("key").map(String::as_str);
  if let Err(error) = client::register(env, id, timeout) {
    let mut saved = client::local(env, id, key, 32)?;
    saved
      .as_object_mut()
      .ok_or("invalid saved status")?
      .insert("registration_error".into(), json!(error));
    return Ok(saved);
  }
  let local = client::sync(env, id, key, 32, timeout)?;
  if let Some(key) = key
    && local
      .get("messages")
      .and_then(Value::as_object)
      .is_some_and(serde_json::Map::is_empty)
  {
    client::request(env, id, "status", json!({"key":key}), timeout)
  } else {
    Ok(local)
  }
}
/// Execute a peer command.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let mut env = Env::process();
  if let Some(dir) = &ctx.config_dir {
    env = env.with("TOOLU_CONFIG_DIR", &dir.to_string_lossy());
  }
  match execute(matches, &env) {
    Ok(value) => Outcome::data(value.to_string()),
    Err(error) => Outcome::failed(Exit::Failure, format!("peers: {error}")),
  }
}

#[cfg(test)]
#[path = "tests/peers_test.rs"]
mod tests;
