//! The projected analysis, in the shape of `fixtures/shell/analysis.json`
//! (`packages/toolu-core/src/shell/__tests__/analysis-projection.ts`).

use serde_json::{Value, json};
use toolu_shell::analysis::{ShellAnalysis, ShellCommand, ShellRedirect};
use toolu_shell::git::{
  Refspec, commit_messages, git_invocation, push_targets, runs_git_subcommand,
};
use toolu_shell::writes::write_targets;

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
  let targets = write_targets(analysis);
  let shown = targets.iter().map(|w| {
    let at = w.command.and_then(|c| index(analysis, c));
    json!({ "path": w.path, "pattern": w.pattern, "text": w.text, "via": w.via.as_str(), "command": at })
  });
  shown.collect()
}

fn git(analysis: &ShellAnalysis) -> (Vec<Value>, Vec<Value>) {
  let (mut invocations, mut messages) = (Vec::new(), Vec::new());
  for (at, c) in analysis.commands.iter().enumerate() {
    let Some(g) = git_invocation(c) else {
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
  let targets = push_targets(analysis);
  let shown = targets.iter().map(|p| {
    let mut push =
      json!({ "command": index(analysis, p.invocation.command), "destination": p.destination });
    match p.refspec {
      Refspec::Absent => {}
      Refspec::Dynamic => push["refspec"] = Value::Null,
      Refspec::Static(spec) => push["refspec"] = json!(spec),
    }
    push
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
