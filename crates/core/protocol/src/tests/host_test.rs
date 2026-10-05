use super::Host;

#[test]
fn every_host_name_parses_back_to_its_host() {
  let names: Vec<&str> = Host::ALL.iter().map(|host| host.name()).collect();
  assert_eq!(names, ["claude", "codex", "opencode", "cursor", "hermes"]);
  for host in Host::ALL {
    assert_eq!(Host::parse(host.name()), Some(host));
  }
}

#[test]
fn unknown_and_differently_cased_names_do_not_parse() {
  for text in ["bogus", "Codex", "CLAUDE", " claude", "open-code", ""] {
    assert_eq!(Host::parse(text), None, "{text:?}");
  }
}
