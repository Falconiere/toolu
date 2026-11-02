use super::*;
use toolu_mux::relay::config::Machine;

#[test]
fn first_attachment_preserves_other_controllers_registrations_and_authority() {
  let machine = |name: &str| Machine {
    name: name.into(),
    root: format!("/srv/{name}"),
    agents: vec!["codex".into()],
    capacity: 1,
    draining: false,
  };
  let mut authoritative = config::Config {
    relay: Some("relay".into()),
    generation: Some("first".into()),
    ..config::Config::default()
  };
  authoritative
    .machines
    .insert("alpha".into(), machine("alpha"));
  let mut requested = config::Config::default();
  requested.machines.insert("beta".into(), machine("beta"));
  let next = merged(&authoritative, &requested);
  assert_eq!(next.generation, Some("first".into()));
  assert_eq!(
    next.machines.keys().map(String::as_str).collect::<Vec<_>>(),
    vec!["alpha", "beta"]
  );
}
