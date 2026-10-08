//! The clap tree of `toolu jev`.

use clap::{Arg, ArgAction, Command};

/// The namespace and its four verbs.
pub(crate) fn command() -> Command {
  Command::new("jev")
    .about("Typed judgments from Jev: yes/no probabilities, choices and scores")
    .subcommand_required(true)
    .arg_required_else_help(true)
    .subcommand(
      shared(Command::new("noul").about("Yes/no judgment: the probability of yes"))
        .arg(instructions())
        .arg(side("true", "What a yes means"))
        .arg(side("false", "What a no means"))
        .arg(id()),
    )
    .subcommand(
      shared(Command::new("choice").about("Pick one option: a choice, probabilities, confidence"))
        .arg(instructions())
        .arg(
          Arg::new("option")
            .short('o')
            .long("option")
            .value_name("KEY[=DESC]")
            .action(ArgAction::Append)
            .help("An option and what it means; repeat 2 to 255 times"),
        )
        .arg(id()),
    )
    .subcommand(
      shared(Command::new("score").about("Rate on ordered levels: a score, legend, confidence"))
        .arg(instructions())
        .arg(
          Arg::new("level")
            .short('l')
            .long("level")
            .value_name("DESC")
            .action(ArgAction::Append)
            .help("A level, lowest first; repeat 2 to 10 times"),
        )
        .arg(id()),
    )
    .subcommand(
      shared(Command::new("ask").about("Many typed questions in one call")).arg(
        Arg::new("questions")
          .value_name("QUESTIONS.JSON")
          .required(true)
          .help("A JSON file of questions by id, or - for stdin"),
      ),
    )
}

/// `--state`, `--model` and `--raw`, which every verb takes.
fn shared(verb: Command) -> Command {
  verb
    .arg(
      Arg::new("state")
        .short('s')
        .long("state")
        .value_name("STATE")
        .required(true)
        .allow_hyphen_values(true)
        .help("State to judge: text, @FILE, or - for stdin"),
    )
    .arg(
      Arg::new("model")
        .short('m')
        .long("model")
        .value_name("MODEL")
        .default_value(toolu_jev_client::DEFAULT_MODEL)
        .help("The Jev model"),
    )
    .arg(
      Arg::new("raw")
        .long("raw")
        .action(ArgAction::SetTrue)
        .help("Print the whole response body instead of its answers"),
    )
}

fn instructions() -> Arg {
  Arg::new("instructions")
    .value_name("INSTRUCTIONS")
    .required(true)
    .help("What to judge")
}

fn side(name: &'static str, help: &'static str) -> Arg {
  Arg::new(name).long(name).value_name("DESC").help(help)
}

fn id() -> Arg {
  Arg::new("id")
    .long("id")
    .value_name("ID")
    .default_value("q")
    .help("The question id in the answer map")
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
