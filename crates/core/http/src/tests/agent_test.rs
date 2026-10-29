use std::time::Duration;

use toolu_runtime::env::Env;

use crate::{Client, Config, Error};

#[test]
fn with_timeout_rejects_zero_without_touching_the_pool() {
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  assert!(matches!(
    client.with_timeout(Duration::ZERO),
    Err(Error::InvalidConfig(_))
  ));
}

#[test]
fn clone_and_with_timeout_share_the_same_agent_pool() {
  let client = Client::new(Config::default(), &Env::default()).expect("client");
  let cloned = client.clone();
  let timed = client
    .with_timeout(Duration::from_secs(5))
    .expect("with_timeout");
  assert!(std::sync::Arc::ptr_eq(&client.agents, &cloned.agents));
  assert!(std::sync::Arc::ptr_eq(&client.agents, &timed.agents));
}
