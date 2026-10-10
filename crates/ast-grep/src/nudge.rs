//! Native search nudge: Bash file searches and structural Grep-tool patterns.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::{Map, Value};
use toolu_protocol::decision::Decision;
use toolu_protocol::normalized::NormalizedEvent;
use toolu_protocol::text::Text;
use toolu_runtime::config::{load, read};
use toolu_runtime::host::roots::Roots;
use toolu_runtime::registry::RegistryEvent;
use toolu_runtime::registry::rule::{Rule, RuleContext};
use toolu_shell::analysis::{ShellAnalysis, ShellCommand};
use toolu_shell::git::git_invocation;
use toolu_state::detect::tools::detect_ast_grep;

use crate::launched::{program_at, program_indexes};

const STRUCT: &str = r"(^|\s)(fn |impl |async fn|async |class |function |struct |trait |interface |type |pub (fn|struct|enum|trait|mod|type|async)|export (function|class|interface|type|const|default)|enum |mod |const fn|#\[derive|#\[cfg|#\[test|@Component|@Injectable|@Module|=>|-> Result|-> impl|dyn |Box<|Arc<|Vec<|Option<|where |for .*in )";
const NON_CODE_GLOB: &str = r"(?i)\*\.(toml|md|markdown|json|jsonc|yaml|yml|txt|env|sql|sh|bash|zsh|fish|lock|cfg|ini|conf|csv|xml|html|svg|css|graphql|gql|proto|makefile|dockerfile)";
const NON_CODE_TYPE: &str = r"(?im)^(toml|json|yaml|md|markdown|html|css|sql|sh|bash|make|docker|config|xml|csv|graphql|proto)$";
const NON_CODE_PATH: &str = r"(?i)(\.claude/|docs/|\.github/|infra/|scripts/|\.config|Makefile|Dockerfile|Cargo\.toml|package\.json|tsconfig)";

#[derive(Clone, Copy)]
enum Kind {
  GrepStructural,
  BashStructural,
  BashGeneric,
}

#[derive(Clone, Copy)]
enum State {
  Available,
  Missing,
  OptOut,
}

fn matches(regex: &'static OnceLock<Option<Regex>>, pattern: &str, text: &str) -> bool {
  regex
    .get_or_init(|| Regex::new(pattern).ok())
    .as_ref()
    .is_some_and(|compiled| compiled.is_match(text))
}

fn structural(text: &str) -> bool {
  static REGEX: OnceLock<Option<Regex>> = OnceLock::new();
  matches(&REGEX, STRUCT, text)
}

fn field(input: &Map<String, Value>, key: &str) -> String {
  match input.get(key) {
    Some(Value::String(text)) => text.trim_end_matches('\n').to_owned(),
    Some(Value::Null | Value::Bool(false)) | None => String::new(),
    Some(other) => serde_json::to_string_pretty(other).unwrap_or_default(),
  }
}

fn grep_tool(input: &Map<String, Value>) -> Option<Kind> {
  static GLOB: OnceLock<Option<Regex>> = OnceLock::new();
  static TYPE: OnceLock<Option<Regex>> = OnceLock::new();
  static PATH: OnceLock<Option<Regex>> = OnceLock::new();
  if matches(&GLOB, NON_CODE_GLOB, &field(input, "glob"))
    || matches(&TYPE, NON_CODE_TYPE, &field(input, "type"))
    || matches(&PATH, NON_CODE_PATH, &field(input, "path"))
  {
    return None;
  }
  structural(&field(input, "pattern")).then_some(Kind::GrepStructural)
}

fn search_index(command: &ShellCommand) -> Option<usize> {
  if git_invocation(command).is_some_and(|git| git.subcommand == Some("grep")) {
    return Some(0);
  }
  program_indexes(command)
    .into_iter()
    .find(|at| matches!(program_at(command, *at), Some("grep" | "rg" | "ripgrep")))
}

fn shell(analysis: &ShellAnalysis) -> Option<Kind> {
  if analysis.unknown {
    return None;
  }
  let searches: Vec<&[String]> = analysis
    .commands
    .iter()
    .filter(|command| {
      command.pipeline.index == 0 || command.wrappers.iter().any(|wrapper| wrapper == "xargs")
    })
    .filter_map(|command| search_index(command).and_then(|at| command.texts.get(at + 1..)))
    .collect();
  if searches.is_empty() {
    return None;
  }
  let is_structural = searches
    .iter()
    .any(|args| args.iter().any(|arg| structural(arg)));
  Some(if is_structural {
    Kind::BashStructural
  } else {
    Kind::BashGeneric
  })
}

fn message(kind: Kind, state: State) -> Option<&'static str> {
  match (kind, state) {
    (Kind::GrepStructural, State::Available) => Some(
      "STOP: Structural code pattern detected. Use ast-grep: `ast-grep run --pattern \"your pattern\" --lang rust/typescript .` AST-aware matching is far more accurate. Grep is for exact literals on non-code files, or after ast-grep returned nothing.",
    ),
    (Kind::GrepStructural, State::Missing) => Some(
      "WARN: structural code pattern detected but ast-grep is not installed. Falling back to Grep — expect false positives. Install ast-grep (`cargo install ast-grep` or `brew install ast-grep`) for AST-aware matching.",
    ),
    (Kind::BashStructural, State::Available) => Some(
      "STOP: grep/rg for structural code search. Use ast-grep: `ast-grep run --pattern \"your pattern\" --lang rust/typescript .` grep/rg is for piping command output or non-code files.",
    ),
    (Kind::BashStructural, State::Missing) => Some(
      "WARN: structural grep/rg detected but ast-grep is not installed. Proceeding with grep/rg — expect false positives. Install ast-grep (`cargo install ast-grep` or `brew install ast-grep`) for AST-aware matching.",
    ),
    (Kind::BashGeneric, State::Available) => Some(
      "grep/rg in Bash detected. Use ast-grep for structural patterns on code files, Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output.",
    ),
    (Kind::BashGeneric, State::Missing) => Some(
      "grep/rg in Bash detected. Use Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output. (ast-grep not installed — structural matching unavailable.)",
    ),
    (Kind::BashGeneric, State::OptOut) => Some(
      "grep/rg in Bash detected. Use Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output.",
    ),
    (Kind::GrepStructural | Kind::BashStructural, State::OptOut) => None,
  }
}

fn state(ctx: &RuleContext<'_>) -> State {
  let roots = Roots::new(ctx.env.clone(), Some(ctx.host));
  let config = load::load(&roots, ctx.cwd);
  if !read::enabled(&config, "skills", "ast-grep") {
    return State::OptOut;
  }
  if detect_ast_grep(ctx.env) {
    State::Available
  } else {
    State::Missing
  }
}

/// The compiled pre-tool rule.
struct SearchNudge;

/// `search-nudge` manifest's compiled rule.
pub const SEARCH_NUDGE: &dyn Rule = &SearchNudge;

impl Rule for SearchNudge {
  fn spec(&self) -> &'static str {
    "ast-grep@toolu"
  }
  fn name(&self) -> &'static str {
    "search-nudge"
  }
  fn event(&self) -> RegistryEvent {
    RegistryEvent::ToolPre
  }

  fn applies(&self, event: &NormalizedEvent, _ctx: &RuleContext<'_>) -> bool {
    event
      .tool()
      .is_some_and(|tool| matches!(tool.name.as_str(), "Grep" | "Bash" | "Shell"))
  }

  fn run(&self, event: &NormalizedEvent, ctx: &RuleContext<'_>) -> Decision {
    let kind = if let NormalizedEvent::ShellPre { command, .. } = event {
      shell(&toolu_shell::analyze(command.as_str()))
    } else if let NormalizedEvent::ToolPre { tool, .. } = event {
      match tool.name.as_str() {
        "Grep" => grep_tool(&tool.input),
        "Bash" | "Shell" => shell(&toolu_shell::analyze(&field(&tool.input, "command"))),
        _ => None,
      }
    } else {
      None
    };
    kind
      .and_then(|kind| message(kind, state(ctx)))
      .and_then(|message| Text::new(message).ok())
      .map_or(Decision::Allow, |message| Decision::Advisory { message })
  }
}

#[cfg(test)]
#[path = "tests/nudge_test.rs"]
mod tests;
