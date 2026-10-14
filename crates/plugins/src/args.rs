//! The clap tree of `toolu plugins` and the request a command line parses into.
//! The verbs and flags are those of the former TypeScript installer, which
//! #438 replaced; `--host` and `--json` are the binary's global flags.

use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

/// What to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Verb {
  /// Install plugins, dependencies first.
  Install,
  /// The catalog joined with the host's state.
  List,
  /// Uninstall plugins.
  Remove,
  /// Update plugins to the marketplace version.
  Update,
}

/// `--scope`: where a host installs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
  /// Claude Code's user scope.
  User,
  /// The project.
  Project,
  /// Claude Code's local scope.
  Local,
}

impl Scope {
  fn parse(text: &str) -> Option<Scope> {
    match text {
      "user" => Some(Scope::User),
      "project" => Some(Scope::Project),
      "local" => Some(Scope::Local),
      _other => None,
    }
  }

  /// The `--scope` value.
  pub(crate) fn name(self) -> &'static str {
    match self {
      Scope::User => "user",
      Scope::Project => "project",
      Scope::Local => "local",
    }
  }
}

/// A parsed `toolu plugins <verb>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Request {
  /// The verb.
  pub(crate) verb: Verb,
  /// The plugin names given.
  pub(crate) names: Vec<String>,
  /// `--scope`.
  pub(crate) scope: Option<Scope>,
  /// `--config`: a `.toolu/plugins.json` to replay (install only).
  pub(crate) config: Option<PathBuf>,
  /// `--yes`.
  pub(crate) yes: bool,
  /// `--dry-run`.
  pub(crate) dry_run: bool,
  /// `--no-input`.
  pub(crate) no_input: bool,
  /// `--catalog`.
  pub(crate) catalog: Option<PathBuf>,
}

const ABOUT: &str = "Install, list, remove and update toolu plugins on every host";

const AFTER: &str = "Claude Code and Codex are driven through their own plugin CLIs. \
  The npm package @toolu/plugins (`npx @toolu/plugins install`) runs these verbs; \
  its guide is docs/cli/installer.md.";

/// The `toolu plugins` namespace.
pub(crate) fn command() -> Command {
  Command::new("plugins")
    .about(ABOUT)
    .after_help(AFTER)
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      changes(Command::new("install").about("Install plugins, dependencies first (no names: all)"))
        .arg(names(
          false,
          "Plugins to install; none means all (a terminal picks)",
        ))
        .arg(
          Arg::new("config")
            .long("config")
            .value_name("PATH")
            .value_parser(value_parser!(PathBuf))
            .conflicts_with("names")
            .help("Replay a .toolu/plugins.json selection"),
        )
        .arg(yes()),
    )
    .subcommand(
      Command::new("list")
        .about("Show the catalog and what the host has installed")
        .arg(catalog()),
    )
    .subcommand(
      changes(Command::new("remove").about("Uninstall plugins (requires --yes)"))
        .arg(names(true, "Plugins to remove"))
        .arg(yes()),
    )
    .subcommand(
      changes(Command::new("update").about("Update plugins to the marketplace version")).arg(
        names(false, "Plugins to update; none means every installed one"),
      ),
    )
}

/// `--catalog`, which every verb takes; the npm wrapper passes its own copy.
fn catalog() -> Arg {
  Arg::new("catalog")
    .long("catalog")
    .value_name("PATH")
    .value_parser(value_parser!(PathBuf))
    .help("Read the plugin catalog from PATH instead of the one built in")
}

/// `--scope`, `--dry-run`, `--no-input` and `--catalog`, which every changing verb takes.
fn changes(verb: Command) -> Command {
  verb
    .arg(catalog())
    .arg(
      Arg::new("scope")
        .long("scope")
        .value_name("SCOPE")
        .value_parser(["user", "project", "local"])
        .help("user, project or local (Claude Code only)"),
    )
    .arg(flag(
      "dry-run",
      "Print the host commands without running them",
    ))
    .arg(flag(
      "no-input",
      "Never prompt; fail naming what is missing",
    ))
}

fn names(required: bool, help: &'static str) -> Arg {
  Arg::new("names")
    .value_name("NAME")
    .num_args(1..)
    .required(required)
    .help(help)
}

fn yes() -> Arg {
  flag("yes", "Confirm a destructive operation").short('y')
}

fn flag(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name)
    .long(name)
    .action(ArgAction::SetTrue)
    .help(help)
}

/// The request of a `toolu plugins` match; `None` without a known verb.
pub(crate) fn request(matches: &ArgMatches) -> Option<Request> {
  let (name, sub) = matches.subcommand()?;
  let verb = match name {
    "install" => Verb::Install,
    "list" => Verb::List,
    "remove" => Verb::Remove,
    "update" => Verb::Update,
    _other => return None,
  };
  let has = |id: &str| {
    sub
      .try_get_one::<bool>(id)
      .ok()
      .flatten()
      .copied()
      .unwrap_or(false)
  };
  let text = |id: &str| sub.try_get_one::<String>(id).ok().flatten().cloned();
  let path = |id: &str| sub.try_get_one::<PathBuf>(id).ok().flatten().cloned();
  let names = sub
    .try_get_many::<String>("names")
    .ok()
    .flatten()
    .map(|values| values.cloned().collect())
    .unwrap_or_default();
  Some(Request {
    verb,
    names,
    scope: text("scope").as_deref().and_then(Scope::parse),
    config: path("config"),
    yes: has("yes"),
    dry_run: has("dry-run"),
    no_input: has("no-input"),
    catalog: path("catalog"),
  })
}

#[cfg(test)]
#[path = "tests/args_test.rs"]
mod tests;
