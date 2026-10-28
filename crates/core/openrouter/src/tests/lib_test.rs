use toolu_runtime::env::Env;

use super::{Client, Config, DEFAULT_MODEL, ENDPOINT, Error};

#[test]
fn the_key_and_model_come_only_from_the_supplied_environment() {
  assert!(matches!(
    Client::from_env(&Env::default(), Config::default()),
    Err(Error::Unconfigured)
  ));
  let env = Env::from_pairs([("OPENROUTER_API_KEY", "key\nvalue")]);
  assert!(matches!(
    Client::from_env(&env, Config::default()),
    Err(Error::InvalidKey)
  ));
  let env = Env::from_pairs([
    ("OPENROUTER_API_KEY", "private-key"),
    ("OPENROUTER_MODEL", "custom/model"),
  ]);
  let client = Client::from_env(&env, Config::default()).expect("client");
  assert_eq!(client.model(), "custom/model");
  assert!(!format!("{client:?}").contains("private-key"));
  let env = Env::from_pairs([("OPENROUTER_API_KEY", "key")]);
  assert_eq!(
    Client::from_env(&env, Config::default())
      .expect("client")
      .model(),
    DEFAULT_MODEL
  );
  assert_eq!(Config::default().endpoint, ENDPOINT);
  let env = Env::from_pairs([("OPENROUTER_API_KEY", "key\u{0}")]);
  assert!(matches!(
    Client::from_env(&env, Config::default()),
    Err(Error::InvalidKey)
  ));
  let plaintext = Config {
    endpoint: "http://openrouter.ai/api/v1/chat/completions".into(),
    ..Config::default()
  };
  assert!(matches!(
    Client::from_env(&Env::from_pairs([("OPENROUTER_API_KEY", "key")]), plaintext),
    Err(Error::InvalidConfig)
  ));
  let env = Env::from_pairs([("OPENROUTER_API_KEY", "key"), ("OPENROUTER_MODEL", "")]);
  assert_eq!(
    Client::from_env(&env, Config::default())
      .expect("client")
      .model(),
    DEFAULT_MODEL
  );
}
