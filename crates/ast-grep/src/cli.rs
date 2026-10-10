//! `toolu ast-grep` verbs, using the external ast-grep CLI as their engine.

use std::path::Path;

use clap::{Arg, ArgMatches, Command};
use serde_json::json;
use toolu_protocol::exit::Exit;
use toolu_runtime::cli::{Ctx, Outcome};
use toolu_runtime::env::Env;
use toolu_runtime::process::{Spec, run as run_process};
use toolu_state::detect::tools::tool_available;

use crate::report::{parse, read_ledger, render};

const LANGS: &[(&str, &str)] = &[
  ("ts", "typescript"),
  ("tsx", "tsx"),
  ("js", "javascript"),
  ("mjs", "javascript"),
  ("cjs", "javascript"),
  ("jsx", "jsx"),
  ("rs", "rust"),
  ("py", "python"),
  ("go", "go"),
  ("rb", "ruby"),
  ("java", "java"),
];

fn external(name: &'static str, about: &'static str) -> Command {
  Command::new(name)
    .about(about)
    .arg(Arg::new("pattern").value_name("PATTERN_OR_RULE"))
    .arg(
      Arg::new("args")
        .num_args(0..)
        .trailing_var_arg(true)
        .allow_hyphen_values(true),
    )
}

/// Search, list matching files, scan a rule, debug a pattern, or report bytes.
pub fn command() -> Command {
  Command::new("ast-grep")
    .about("Structural code search and rewrite with ast-grep")
    .subcommand_required(true)
    .subcommand(external("search", "Search for a structural pattern"))
    .subcommand(external("files", "List matching files"))
    .subcommand(external("scan", "Scan with inline YAML or a rule file"))
    .subcommand(external("debug", "Print a pattern's syntax tree"))
    .subcommand(
      Command::new("savings")
        .about("Summarize a byte-savings JSONL ledger")
        .arg(Arg::new("ledger").value_name("LEDGER.jsonl").required(true)),
    )
}

fn values(matches: &ArgMatches) -> Vec<String> {
  matches
    .get_many::<String>("args")
    .into_iter()
    .flatten()
    .cloned()
    .collect()
}

fn inferred_lang(args: &[String]) -> Vec<String> {
  if args.iter().any(|arg| {
    arg == "--lang" || arg == "-l" || arg.starts_with("--lang=") || arg.starts_with("-l=")
  }) {
    return Vec::new();
  }
  let first_path = args.iter().find(|arg| Path::new(arg).exists());
  let Some(file) = first_path.filter(|path| Path::new(path).is_file()) else {
    return Vec::new();
  };
  let ext = Path::new(file).extension().and_then(|ext| ext.to_str());
  LANGS
    .iter()
    .find(|(name, _)| Some(*name) == ext)
    .map_or_else(Vec::new, |(_, lang)| {
      vec!["--lang".to_owned(), (*lang).to_owned()]
    })
}

fn argv(verb: &str, matches: &ArgMatches) -> Result<Vec<String>, String> {
  let pattern = matches
    .get_one::<String>("pattern")
    .cloned()
    .unwrap_or_default();
  if pattern.is_empty() {
    let object = if verb == "scan" {
      "inline YAML or rule file path"
    } else {
      "a pattern"
    };
    return Err(format!("{verb} requires {object}"));
  }
  let args = values(matches);
  if verb == "scan" {
    let mode = if Path::new(&pattern).is_file() {
      "--rule"
    } else {
      "--inline-rules"
    };
    let mut line = vec!["scan".to_owned(), mode.to_owned(), pattern];
    line.extend(
      [
        "--report-style",
        "short",
        "--max-results",
        "50",
        "--color",
        "never",
      ]
      .map(str::to_owned),
    );
    line.extend(args);
    return Ok(line);
  }
  let mut line = vec!["run".to_owned(), "--pattern".to_owned(), pattern];
  if verb == "files" {
    line.push("--files-with-matches".to_owned());
  }
  if verb == "debug" {
    line.push("--debug-query=pattern".to_owned());
  }
  line.extend(["--color".to_owned(), "never".to_owned()]);
  line.extend(inferred_lang(&args));
  line.extend(args);
  Ok(line)
}

fn result(verb: &str, stdout: &str, stderr: &str, code: i32, json_output: bool) -> Outcome {
  let exit = if code == 0 {
    Exit::Success
  } else {
    Exit::Failure
  };
  if json_output {
    return Outcome {
      exit,
      stdout: Some(
        json!({
          "verb": verb, "stdout": stdout, "stderr": stderr, "exitCode": code
        })
        .to_string(),
      ),
      stderr: None,
    };
  }
  Outcome {
    exit,
    stdout: (!stdout.is_empty()).then(|| stdout.strip_suffix('\n').unwrap_or(stdout).to_owned()),
    stderr: (!stderr.is_empty()).then(|| stderr.strip_suffix('\n').unwrap_or(stderr).to_owned()),
  }
}

fn execute(verb: &str, matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let env = Env::process();
  let binary = if tool_available("sg", &env) {
    Some("sg")
  } else if tool_available("ast-grep", &env) {
    Some("ast-grep")
  } else {
    None
  };
  let Some(binary) = binary else {
    return result(verb, "", "", 0, ctx.json);
  };
  let args = match argv(verb, matches) {
    Ok(args) => args,
    Err(error) => return Outcome::failed(Exit::Usage, format!("ast-grep: {error}")),
  };
  let mut spec = Spec::new([binary.to_owned()]);
  spec.argv.extend(args);
  spec.env = Some(env);
  match run_process(&spec) {
    Ok(output) => result(
      verb,
      &output.stdout,
      &output.stderr,
      output.exit_code,
      ctx.json,
    ),
    Err(error) => Outcome::failed(Exit::Unavailable, format!("ast-grep: {error:?}")),
  }
}

fn savings(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  let Some(ledger) = matches.get_one::<String>("ledger") else {
    return Outcome::failed(
      Exit::Usage,
      "usage: toolu ast-grep savings <ledger.jsonl>".to_owned(),
    );
  };
  let path = Path::new(ledger);
  if !path.is_file() {
    return Outcome::failed(
      Exit::Usage,
      "usage: toolu ast-grep savings <ledger.jsonl>".to_owned(),
    );
  }
  match read_ledger(path) {
    Ok(text) => match parse(&text) {
      Ok(records) => result("savings", &render(&records), "", 0, ctx.json),
      Err(line) => Outcome::failed(
        Exit::Failure,
        format!("byte-savings-report: {ledger}:{line}: invalid ledger line"),
      ),
    },
    Err(error) => Outcome::failed(
      Exit::Failure,
      format!("byte-savings-report: {ledger}: {error}"),
    ),
  }
}

/// Run a namespace verb with the CLI's output and exit contract.
pub fn run(matches: &ArgMatches, ctx: &Ctx) -> Outcome {
  match matches.subcommand() {
    Some((verb @ ("search" | "files" | "scan" | "debug"), args)) => execute(verb, args, ctx),
    Some(("savings", args)) => savings(args, ctx),
    _ => Outcome::failed(Exit::Usage, "toolu ast-grep: unknown verb".to_owned()),
  }
}

#[cfg(test)]
#[path = "tests/cli_test.rs"]
mod tests;
