use std::path::PathBuf;

use clap::error::ErrorKind;

use super::{Request, Scope, Verb, command, request};

fn parse(words: &[&str]) -> Result<Request, ErrorKind> {
  let argv = std::iter::once("plugins").chain(words.iter().copied());
  let matches = command()
    .try_get_matches_from(argv)
    .map_err(|err| err.kind())?;
  request(&matches).ok_or(ErrorKind::InvalidSubcommand)
}

#[test]
fn reads_a_full_install_invocation() {
  let words = [
    "install",
    "--catalog",
    "/tmp/m.json",
    "rust-quality",
    "ts-quality",
    "--scope",
    "project",
    "-y",
    "--dry-run",
    "--no-input",
  ];
  assert_eq!(
    parse(&words),
    Ok(Request {
      verb: Verb::Install,
      names: vec!["rust-quality".to_owned(), "ts-quality".to_owned()],
      scope: Some(Scope::Project),
      config: None,
      yes: true,
      dry_run: true,
      no_input: true,
      catalog: Some(PathBuf::from("/tmp/m.json")),
    })
  );
}

#[test]
fn every_verb_parses_and_takes_the_catalog_flag() {
  let list = parse(&["list", "--catalog", "c.json"]);
  assert_eq!(
    list.as_ref().map(|r| (r.verb, r.catalog.clone())),
    Ok((Verb::List, Some("c.json".into())))
  );
  let update = parse(&["update"]);
  assert_eq!(
    update.as_ref().map(|r| (r.verb, r.names.len())),
    Ok((Verb::Update, 0))
  );
  let remove = parse(&["remove", "jev", "--yes", "--scope", "user"]);
  assert_eq!(
    remove.map(|r| (r.verb, r.names, r.yes, r.scope.map(Scope::name))),
    Ok((Verb::Remove, vec!["jev".to_owned()], true, Some("user")))
  );
  let config = parse(&["install", "--config", ".toolu/plugins.json"]);
  assert_eq!(
    config.map(|r| r.config),
    Ok(Some(PathBuf::from(".toolu/plugins.json")))
  );
}

#[test]
fn malformed_command_lines_are_clap_usage_errors() {
  assert_eq!(parse(&["skills"]), Err(ErrorKind::InvalidSubcommand));
  assert_eq!(parse(&["remove"]), Err(ErrorKind::MissingRequiredArgument));
  assert_eq!(
    parse(&["install", "--scope", "global"]),
    Err(ErrorKind::InvalidValue)
  );
  assert_eq!(
    parse(&["install", "--host-x"]),
    Err(ErrorKind::UnknownArgument)
  );
  assert_eq!(
    parse(&["install", "jev", "--config", "p.json"]),
    Err(ErrorKind::ArgumentConflict)
  );
  assert_eq!(
    parse(&["list", "--config", "p.json"]),
    Err(ErrorKind::UnknownArgument)
  );
  assert_eq!(
    parse(&["list", "--dry-run"]),
    Err(ErrorKind::UnknownArgument)
  );
  assert_eq!(parse(&["update", "--yes"]), Err(ErrorKind::UnknownArgument));
}

#[test]
fn every_scope_value_round_trips() {
  for scope in [Scope::User, Scope::Project, Scope::Local] {
    assert_eq!(Scope::parse(scope.name()), Some(scope));
  }
  assert_eq!(Scope::parse("global"), None);
}
