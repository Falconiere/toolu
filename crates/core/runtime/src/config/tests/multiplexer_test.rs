use serde_json::{Map, Value, json};
use toolu_protocol::host::Host;

use super::Multiplexer;
use crate::config::load::LoadedConfig;

fn config(value: Value) -> LoadedConfig {
  let mut data = Map::new();
  data.insert("multiplexer".to_owned(), value);
  LoadedConfig::from_data(data, Host::Codex)
}

#[test]
fn the_default_is_auto() {
  let empty = LoadedConfig::from_data(Map::new(), Host::Codex);
  assert_eq!(Multiplexer::parse(&empty).unwrap(), Multiplexer::Auto);
}

#[test]
fn each_named_backend_parses() {
  assert_eq!(
    Multiplexer::parse(&config(json!("auto"))).unwrap(),
    Multiplexer::Auto
  );
  assert_eq!(
    Multiplexer::parse(&config(json!("herdr"))).unwrap(),
    Multiplexer::Herdr
  );
  assert_eq!(
    Multiplexer::parse(&config(json!("tmux"))).unwrap(),
    Multiplexer::Tmux
  );
}

#[test]
fn an_unknown_value_is_rejected() {
  let err = Multiplexer::parse(&config(json!("screen"))).unwrap_err();
  assert_eq!(err, "multiplexer must be auto, herdr, herdr-app or tmux");
  let err = Multiplexer::parse(&config(json!(1))).unwrap_err();
  assert_eq!(err, "multiplexer must be auto, herdr, herdr-app or tmux");
}

#[test]
fn as_str_round_trips_through_parse() {
  for value in [
    Multiplexer::Auto,
    Multiplexer::Herdr,
    Multiplexer::HerdrApp,
    Multiplexer::Tmux,
  ] {
    assert_eq!(
      Multiplexer::parse(&config(json!(value.as_str()))).unwrap(),
      value
    );
  }
}
