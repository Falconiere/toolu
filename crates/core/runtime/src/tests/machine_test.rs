use super::{short, short_hostname};

#[test]
fn short_hostname_matches_the_hostname_command() {
  let output = std::process::Command::new("hostname")
    .output()
    .expect("hostname");
  let name = String::from_utf8(output.stdout).expect("utf-8");
  let expected = name.trim().split('.').next().unwrap_or("").to_owned();
  assert_eq!(short_hostname(), Some(expected));
}

#[test]
fn short_cuts_at_the_first_dot_and_refuses_an_empty_name() {
  assert_eq!(short("studio-mbp.local"), Some("studio-mbp".to_owned()));
  assert_eq!(short("build-mini"), Some("build-mini".to_owned()));
  assert_eq!(short(""), None);
  assert_eq!(short(".local"), None);
}
