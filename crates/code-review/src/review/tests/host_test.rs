use super::{model_options, pick_host, pick_model, premium_model};
use toolu_protocol::host::Host;

#[test]
fn host_choice_prefers_jev_then_default_then_first() {
  let two = [Host::Codex, Host::Muse];
  assert_eq!(pick_host(&two, Some("muse"), Host::Codex), Some(Host::Muse));
  assert_eq!(
    pick_host(&two, Some("claude"), Host::Codex),
    Some(Host::Codex)
  );
  assert_eq!(pick_host(&two, None, Host::Claude), Some(Host::Codex));
  assert_eq!(pick_host(&[], Some("muse"), Host::Codex), None);
}

#[test]
fn model_choice_only_names_a_verified_premium() {
  for host in Host::ALL {
    let options = model_options(host);
    assert_eq!(options.first().map(|(key, _)| *key), Some("default"));
    assert_eq!(pick_model(host, None), "default");
    assert_eq!(pick_model(host, Some("default")), "default");
  }
  for host in Host::ALL {
    if let Some(premium) = premium_model(host) {
      assert!(options_contain(host, premium));
      assert_eq!(pick_model(host, Some(premium)), premium);
    }
    assert_eq!(pick_model(host, Some("evil-model; rm -rf")), "default");
  }
}

fn options_contain(host: Host, key: &str) -> bool {
  model_options(host).iter().any(|(name, _)| *name == key)
}
