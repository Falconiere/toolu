//! The projected analysis, in the shape of `fixtures/shell/analysis.json`
//! (`packages/toolu-core/src/shell/__tests__/analysis-projection.ts`).

use serde_json::{Value, json};
use toolu_shell::analysis::{ShellAnalysis, ShellCommand, ShellRedirect};
use toolu_shell::git::{
  GitInvocation, GitPush, Refspec, commit_messages, git_invocation, push_targets,
  runs_git_subcommand,
};
use toolu_shell::writes::{WriteTarget, write_targets};

fn redirect(r: &ShellRedirect) -> Value {
  let heredoc = r
    .heredoc
    .as_ref()
    .map(|body| json!({ "content": body.content, "quoted": body.quoted }));
  json!({
    "operator": r.operator.as_str(),
    "fd": r.fd,
    "target": r.target,
    "pattern": r.pattern,
    "text": r.text,
    "heredoc": heredoc,
  })
}

fn command(c: &ShellCommand) -> Value {
  json!({
    "words": c.words,
    "argv": c.argv,
    "patterns": c.patterns,
    "texts": c.texts,
    "wrappers": c.wrappers,
    "redirects": c.redirects.iter().map(redirect).collect::<Vec<_>>(),
    "pipeline": { "index": c.pipeline.index, "size": c.pipeline.size },
    "exitProves": c.exit_proves,
    "origin": c.origin.as_str(),
    "depth": c.depth,
    "text": c.text,
  })
}

/// The index of `target` among the analysis's commands.
fn index(analysis: &ShellAnalysis, target: &ShellCommand) -> Option<usize> {
  analysis
    .commands
    .iter()
    .position(|c| std::ptr::eq(c, target))
}

fn writes(analysis: &ShellAnalysis) -> Vec<Value> {
  let targets: Vec<WriteTarget<'_>> = write_targets(analysis);
  let shown = targets.iter().map(|w| {
    let at = w.command.and_then(|c| index(analysis, c));
    json!({ "path": w.path, "pattern": w.pattern, "text": w.text, "via": w.via.as_str(), "command": at })
  });
  shown.collect()
}

fn git(analysis: &ShellAnalysis) -> (Vec<Value>, Vec<Value>) {
  let (mut invocations, mut messages) = (Vec::new(), Vec::new());
  for (at, c) in analysis.commands.iter().enumerate() {
    let found: Option<GitInvocation<'_>> = git_invocation(c);
    let Some(g) = found else {
      continue;
    };
    invocations.push(
      json!({ "command": at, "subcommand": g.subcommand, "args": g.args, "cChain": g.c_chain }),
    );
    if g.subcommand == Some("commit") {
      messages.push(json!(commit_messages(&g)));
    }
  }
  (invocations, messages)
}

fn pushes(analysis: &ShellAnalysis) -> Vec<Value> {
  let targets: Vec<GitPush<'_>> = push_targets(analysis);
  let shown = targets.iter().map(|p| {
    let at = index(analysis, p.invocation.command);
    let mut push = serde_json::Map::new();
    push.insert("command".to_owned(), json!(at));
    push.insert("destination".to_owned(), json!(p.destination));
    match p.refspec {
      Refspec::Absent => {}
      Refspec::Dynamic => drop(push.insert("refspec".to_owned(), Value::Null)),
      Refspec::Static(spec) => drop(push.insert("refspec".to_owned(), json!(spec))),
    }
    Value::Object(push)
  });
  shown.collect()
}

/// Everything the crate answers for `analysis`.
pub(crate) fn project(analysis: &ShellAnalysis) -> Value {
  let (git, messages) = git(analysis);
  json!({
    "unknown": analysis.unknown,
    "errored": !analysis.errors.is_empty(),
    "commands": analysis.commands.iter().map(command).collect::<Vec<_>>(),
    "compoundRedirects": analysis.compound_redirects.iter().map(redirect).collect::<Vec<_>>(),
    "writes": writes(analysis),
    "git": git,
    "pushes": pushes(analysis),
    "commitMessages": messages,
    "runs": {
      "push": runs_git_subcommand(analysis, "push").as_str(),
      "commit": runs_git_subcommand(analysis, "commit").as_str(),
    },
  })
}
