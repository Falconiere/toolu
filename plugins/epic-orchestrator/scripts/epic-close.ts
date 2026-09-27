/** Close a completed epic in its tracker: post the sub-issue -> merged PR
 * summary, then close (GitHub), transition to Done (Jira), or complete
 * (Linear parent issue). Refuses while any sub-issue is still open. */

import { join } from "node:path";
import { readJson } from "./common.ts";
import { detectTracker, makeTracker } from "./trackers/index.ts";
import type { PrNode } from "./trackers/types.ts";

type Graph = {
  tracker?: string;
  default_repo?: string | null;
  state_dir: string;
  complete: boolean;
  epic: { ref: string };
  issues: { ref: string; key: string; title: string; state: string; prs: PrNode[] }[];
};

type Rec = { last_gate?: { admin_used?: boolean } };

export function summaryTable(graph: Graph, records: Record<string, Rec>): string {
  const rows = graph.issues.map((i) => {
    const merged = i.prs.filter((p) => p.state === "MERGED").map((p) => p.url);
    const admin = records[i.key]?.last_gate?.admin_used ? " (admin merge)" : "";
    const prs = merged.length ? merged.join(", ") : "closed without a merged PR";
    return `| ${i.ref} — ${i.title.replace(/\|/g, "\\|")} | ${prs}${admin} |`;
  });
  return ["Epic complete.", "", "| Sub-issue | Merged PR |", "|---|---|", ...rows].join("\n");
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  let graphPath: string | undefined;
  let dry = false;
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--graph") graphPath = argv[++i];
    else if (a === "--dry-run") dry = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!graphPath) throw new Error("usage: epic-close.ts --graph GRAPH.json [--dry-run]");
  const graph = readJson<Graph | null>(graphPath, null);
  if (!graph) throw new Error(`cannot read graph ${graphPath}`);
  if (!graph.complete) {
    const open = graph.issues.filter((i) => i.state !== "closed").map((i) => i.ref);
    throw new Error(`epic not complete; still open: ${open.join(", ")}. Re-run the graph first.`);
  }
  const records = Object.fromEntries(
    graph.issues.map((i) => [
      i.key,
      readJson<Rec>(join(graph.state_dir, "issues", `${i.key}.json`), {}),
    ]),
  );
  const summary = summaryTable(graph, records);
  if (dry) {
    process.stdout.write(summary + "\n");
    return;
  }
  const kind = detectTracker(graph.epic.ref, graph.tracker);
  const tracker = makeTracker(kind, graph.epic.ref, graph.default_repo ?? "");
  const result = await tracker.closeEpic(summary);
  process.stdout.write(
    JSON.stringify({ epic: graph.epic.ref, tracker: kind, result }, null, 2) + "\n",
  );
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    process.stderr.write(String(err instanceof Error ? err.message : err) + "\n");
    process.exit(1);
  });
}
