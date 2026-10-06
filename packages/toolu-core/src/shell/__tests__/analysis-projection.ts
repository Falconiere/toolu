/**
 * The projected analysis shared with the Rust port (#416): everything
 * `@toolu/core/shell` answers for one command line, as plain JSON. Parser
 * error text and offsets are left out; they are specific to unbash.
 * `fixtures/shell/analysis.json` was captured once from this function.
 */
import { commitMessages, gitInvocation, pushTargets, runsGitSubcommand } from "../shell-git.ts";
import { analyzeShell } from "../shell-parse.ts";
import type { ShellCommand, ShellRedirect } from "../shell-types.ts";
import { writeTargets } from "../shell-writes.ts";

function redirect(r: ShellRedirect) {
  return {
    operator: r.operator,
    fd: r.fd,
    target: r.target,
    pattern: r.pattern,
    text: r.text,
    heredoc: r.heredoc === null ? null : { content: r.heredoc.content, quoted: r.heredoc.quoted },
  };
}

function command(c: ShellCommand) {
  return {
    words: c.words,
    argv: c.argv,
    patterns: c.patterns,
    texts: c.texts,
    wrappers: c.wrappers,
    redirects: c.redirects.map(redirect),
    pipeline: { index: c.pipeline.index, size: c.pipeline.size },
    exitProves: c.exitProves,
    origin: c.origin,
    depth: c.depth,
    text: c.text,
  };
}

/** The projection of `analyzeShell(source)` and every helper over it. */
export function projectAnalysis(source: string): unknown {
  const analysis = analyzeShell(source);
  const index = new Map(analysis.commands.map((c, i) => [c, i]));
  const at = (c: ShellCommand | null) => (c === null ? null : (index.get(c) ?? null));
  const invocations = analysis.commands.map((c) => gitInvocation(c));
  return {
    unknown: analysis.unknown,
    errored: analysis.errors.length > 0,
    commands: analysis.commands.map(command),
    compoundRedirects: analysis.compoundRedirects.map(redirect),
    writes: writeTargets(analysis).map((w) => ({
      path: w.path,
      pattern: w.pattern,
      text: w.text,
      via: w.via,
      command: at(w.command),
    })),
    git: invocations.flatMap((g, i) =>
      g === undefined
        ? []
        : [{ command: i, subcommand: g.subcommand, args: g.args, cChain: g.cChain }],
    ),
    pushes: pushTargets(analysis).map((p) => ({
      command: at(p.invocation.command),
      ...(p.refspec === undefined ? {} : { refspec: p.refspec }),
      destination: p.destination,
    })),
    commitMessages: invocations.flatMap((g) =>
      g?.subcommand === "commit" ? [commitMessages(g)] : [],
    ),
    runs: {
      push: runsGitSubcommand(analysis, "push"),
      commit: runsGitSubcommand(analysis, "commit"),
    },
  };
}
