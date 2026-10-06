//! Simple commands (`emitCommand` in `shell-walk.ts`). Nested scripts are walked
//! first (assignments, then words, then redirects), then wrappers are peeled and
//! a `bash -c`/`eval` string is analyzed in turn.
//!
//! Forms unbash reads as plain commands are folded in here: `declaration_command`
//! and `unset_command` (their keyword is the first word), `[ … ]`, assignment-only
//! lines, and the keywords `time` and `coproc` before a command.

mod gather;

use tree_sitter::Node;

use crate::analysis::{
  CommandOrigin, PipelinePosition, ShellCommand, ShellError, ShellRedirect, Word,
};
use crate::argv;
use crate::redirect;
use crate::walk::{Ctx, MAX_RUN_DEPTH, Walker};
use crate::words::{Resolved, text_of};
use gather::{Gathered, WordNodes};

/// Reserved words that close a construct: bash reads one as a command name as a syntax error.
const CLOSERS: &[&str] = &["then", "else", "elif", "fi", "do", "done", "esac", "}"];

/// Whether `node` is a command unbash reads as a `Command`.
pub(crate) fn is_simple(node: Node<'_>) -> bool {
  match node.kind() {
    "command"
    | "declaration_command"
    | "unset_command"
    | "variable_assignment"
    | "variable_assignments" => true,
    "test_command" => !is_double_bracket(node),
    _ => false,
  }
}

fn is_double_bracket(node: Node<'_>) -> bool {
  node.child(0).is_some_and(|first| first.kind() == "[[")
}

/// `(( … ))`, which tree-sitter-bash 0.23 reads as a command named by an arithmetic expansion.
fn is_arithmetic(node: Node<'_>, source: &str) -> bool {
  let name = node
    .child_by_field_name("name")
    .and_then(|name| name.named_child(0));
  name.is_some_and(|inner| {
    inner.kind() == "arithmetic_expansion" && text_of(inner, source).starts_with("((")
  })
}

/// A command no static reading can name (`bash -c "$CMD"`, recursion too deep).
pub(crate) fn unknown(text: &str, origin: CommandOrigin, depth: usize) -> ShellCommand {
  ShellCommand {
    words: vec![None],
    argv: vec![None],
    patterns: vec![None],
    texts: vec![text.to_owned()],
    wrappers: Vec::new(),
    redirects: Vec::new(),
    pipeline: PipelinePosition::ALONE,
    exit_proves: false,
    origin,
    depth,
    text: text.to_owned(),
  }
}

/// Resolve every word, walking its nested scripts first.
fn resolve_all(walker: &mut Walker, words: &[WordNodes<'_>], ctx: Ctx<'_>) -> Vec<Resolved> {
  let mut resolved = Vec::with_capacity(words.len());
  for word in words {
    for node in &word.nodes {
      walker.visit_nested(*node, ctx);
    }
    resolved.push(word.resolve(ctx.source));
  }
  resolved
}

/// Read every redirect, walking the scripts its target or body runs.
fn read_redirects(
  walker: &mut Walker,
  gathered: &Gathered<'_>,
  ctx: Ctx<'_>,
) -> Vec<ShellRedirect> {
  let mut records = Vec::new();
  for node in &gathered.redirects {
    if let Some(mut found) = redirect::read(*node, ctx.source) {
      for nested in &found.nested {
        walker.visit_nested(*nested, ctx);
      }
      let glued = gathered.fds.iter().find(|(at, _)| *at == node.start_byte());
      found.record.fd = found.record.fd.or(glued.map(|(_, fd)| *fd));
      records.push(found.record);
    }
  }
  records
}

/// Build the record from resolved words.
fn record(
  resolved: Vec<Resolved>,
  redirects: Vec<ShellRedirect>,
  text: &str,
  ctx: Ctx<'_>,
) -> ShellCommand {
  let words: Vec<Word> = resolved.iter().map(|word| word.value.clone()).collect();
  let unwrapped = argv::unwrap(&words);
  let patterns: Vec<Option<String>> = resolved.iter().map(|word| word.pattern.clone()).collect();
  let texts: Vec<String> = resolved.into_iter().map(|word| word.text).collect();
  let proves = ctx.proves && !unwrapped.wrappers.iter().any(|name| name == "xargs");
  ShellCommand {
    argv: argv::align(&words, &unwrapped, &None),
    patterns: argv::align(&patterns, &unwrapped, &None),
    texts: argv::align(&texts, &unwrapped, &String::new()),
    words,
    wrappers: unwrapped.wrappers,
    redirects,
    pipeline: ctx.pipeline,
    exit_proves: proves,
    origin: ctx.origin,
    depth: ctx.depth,
    text: text.to_owned(),
  }
}

/// Analyze the string a shell or `eval` runs, or report it unknown.
fn run_string(walker: &mut Walker, command: &ShellCommand, ctx: Ctx<'_>) {
  let Some(target) = argv::run_target(&command.argv) else {
    return;
  };
  let script = if target.stdin {
    redirect::stdin_script(&command.redirects)
  } else {
    target.script
  };
  let ctx = Ctx {
    proves: command.exit_proves,
    ..ctx
  };
  match script {
    Some(script) if ctx.depth < MAX_RUN_DEPTH => walker.run_string(&script, target.origin, ctx),
    _ => walker
      .commands
      .push(unknown(&command.text, target.origin, ctx.depth + 1)),
  }
}

/// Drop `coproc`, which unbash reads apart from the command: the command runs
/// in the background, so it proves nothing. (`time` is blanked before parsing.)
fn strip_coproc<'w, 't>(words: &'w [WordNodes<'t>], ctx: &mut Ctx<'_>) -> &'w [WordNodes<'t>] {
  let first = words.first().and_then(|word| word.keyword(ctx.source));
  if words.len() < 2 || first != Some("coproc") {
    return words;
  }
  *ctx = ctx.off();
  words.get(1..).unwrap_or_default()
}

/// Emit the simple command `node`, with `statement` redirects written after it.
pub(crate) fn emit(walker: &mut Walker, node: Node<'_>, statement: &[Node<'_>], ctx: Ctx<'_>) {
  if is_double_bracket(node) || is_arithmetic(node, ctx.source) {
    walker.visit_nested(node, ctx);
    return;
  }
  let gathered = gather::gather(node, statement, ctx.source);
  if let Some(closer) = gathered
    .words
    .first()
    .and_then(|word| word.keyword(ctx.source))
    .filter(|word| CLOSERS.contains(word))
  {
    let message = format!("unexpected token '{closer}'");
    walker.errors.push(ShellError {
      message,
      pos: node.start_byte(),
      origin: ctx.origin,
    });
    return;
  }
  for assignment in &gathered.assignments {
    walker.visit_nested(*assignment, ctx);
  }
  let mut ctx = ctx;
  let words = strip_coproc(&gathered.words, &mut ctx);
  let empty = words.is_empty() && gathered.assignments.is_empty() && gathered.redirects.is_empty();
  if empty {
    walk_others(walker, &gathered, ctx);
    return;
  }
  let start = match (words.len() < gathered.words.len(), words.first()) {
    (true, Some(first)) => first.start,
    _ => node.start_byte(),
  };
  let resolved = resolve_all(walker, words, ctx);
  let redirects = read_redirects(walker, &gathered, ctx);
  let text = ctx.source.get(start..gathered.end).unwrap_or_default();
  let command = record(resolved, redirects, text, ctx);
  walker.commands.push(command.clone());
  run_string(walker, &command, ctx);
  walk_others(walker, &gathered, ctx);
}

/// Statements tree-sitter hung under a command (`time (git push)`): walked in turn.
fn walk_others(walker: &mut Walker, gathered: &Gathered<'_>, ctx: Ctx<'_>) {
  for other in &gathered.others {
    walker.walk_statement(*other, ctx);
  }
}

#[cfg(test)]
#[path = "tests/command_test.rs"]
mod tests;
