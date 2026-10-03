/** Prepare or reuse an issue checkout and its verified Herdr workspace. */
import { join } from "node:path";
import { realpathSync } from "node:fs";
import { CommandError, herdr, run } from "./common.ts";
import type { Graph, GraphIssue } from "./launch-issue.ts";

export function shellJoin(argv: string[]): string {
  return argv
    .map((a) => {
      if (a === "") return "''";
      if (/^[A-Za-z0-9_./:=,@%+-]+$/.test(a)) return a;
      return `'${a.replace(/'/g, `'\\''`)}'`;
    })
    .join(" ");
}

export async function ensureWorktree(
  checkout: string,
  issue: GraphIssue,
  base: string,
  dry: boolean,
  log: string[],
): Promise<{ workspace_id: string; pane_id: string; worktree: string }> {
  type Wt = {
    branch?: string;
    open_workspace_id?: string;
    path?: string;
  };
  const listed = dry
    ? []
    : (((await herdr(["worktree", "list", "--cwd", checkout])).worktrees as Wt[] | undefined) ??
      []);
  const existing = listed.find((w) => w.branch === issue.branch);
  if (existing?.open_workspace_id) {
    const ws = existing.open_workspace_id;
    const panes = (await herdr(["pane", "list", "--workspace", ws])).panes as {
      pane_id: string;
    }[];
    const pane0 = panes[0];
    if (!pane0) throw new CommandError(`no panes in workspace ${ws}`);
    log.push(`reuse open worktree workspace ${ws}`);
    return {
      workspace_id: ws,
      pane_id: pane0.pane_id,
      worktree: realpathSync(existing.path ?? ""),
    };
  }
  let cmd: string[];
  if (existing) {
    cmd = [
      "worktree",
      "open",
      "--cwd",
      checkout,
      "--path",
      existing.path ?? "",
      "--label",
      issue.key,
      "--no-focus",
    ];
  } else {
    cmd = [
      "worktree",
      "create",
      "--cwd",
      checkout,
      "--branch",
      issue.branch,
      "--base",
      `origin/${base}`,
      "--label",
      issue.key,
      "--no-focus",
    ];
  }
  log.push("herdr " + shellJoin(cmd));
  if (dry) {
    return { workspace_id: "<new>", pane_id: "<root-pane>", worktree: "<herdr worktree path>" };
  }
  const res = await herdr(cmd);
  const workspace = res.workspace as { workspace_id: string };
  const rootPane = res.root_pane as { pane_id: string };
  const worktree = res.worktree as { path: string };
  return {
    workspace_id: workspace.workspace_id,
    pane_id: rootPane.pane_id,
    worktree: realpathSync(worktree.path),
  };
}

export async function prepareCheckout(
  graph: Graph,
  issue: GraphIssue,
  dry: boolean,
  log: string[],
): Promise<[string, string]> {
  let checkout = issue.checkout;
  if (!checkout) {
    const parts = issue.repo.split("/");
    const repoName = parts[1];
    if (!repoName || parts.length !== 2) throw new Error(`bad repo: ${issue.repo}`);
    checkout = join(graph.clone_root, repoName);
    const cmd = ["gh", "repo", "clone", issue.repo, checkout];
    log.push(shellJoin(cmd));
    if (!dry) await run(cmd);
  }
  // Dry-run must not call the network: CI and offline dry-runs have no access
  // to every epic repo. Live launches still resolve the real default branch.
  let base = "main";
  if (!dry) {
    base = (
      await run([
        "gh",
        "repo",
        "view",
        issue.repo,
        "--json",
        "defaultBranchRef",
        "-q",
        ".defaultBranchRef.name",
      ])
    ).trim();
  } else {
    log.push(`# dry-run: skip gh repo view; assume default branch ${base}`);
  }
  const fetch = ["git", "-C", checkout, "fetch", "origin", base];
  log.push(shellJoin(fetch));
  if (!dry) await run(fetch);
  return [checkout, base];
}
