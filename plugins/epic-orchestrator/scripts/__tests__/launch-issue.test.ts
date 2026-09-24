/** Launcher tests: brief rendering and a real read-only dry run against the #248 snapshot. */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { findIssue, renderBrief } from "../launch-issue.ts";

const HERE = join(import.meta.dir, "..");
const FIXTURE = join(HERE, "fixtures", "epic248-graph.json");
const LAUNCH = join(HERE, "launch-issue.ts");

type Graph = Parameters<typeof findIssue>[0];

describe("BriefTest", () => {
  const graph = JSON.parse(readFileSync(FIXTURE, "utf8")) as Graph;

  test("every placeholder is filled", () => {
    const issue = findIssue(graph, "Falconiere/comemory#255");
    const paths = { worktree: "/wt/comemory/feat-255", status: "/state/status/comemory-255.json" };
    const brief = renderBrief(graph, issue, paths, "main");
    expect(brief).not.toContain("{{");
    expect(brief).toContain("Closes Falconiere/comemory#255");
    expect(brief).toContain("Part of Falconiere/comemory#248");
    expect(brief).toContain(join(HERE, "report.sh"));
    for (const phase of [
      "report brainstorm",
      "report spec",
      "report spec-review",
      "report plan",
      "report plan-review",
      "report execution",
      "/delivery-flow:delivery-flow",
      "/pr-babysit:babysit",
    ]) {
      expect(brief).toContain(phase);
    }
  });

  test("find_issue by key or ref", () => {
    expect(findIssue(graph, "comemory-io-183").ref).toBe("CodaSignal/comemory.io#183");
    expect(() => findIssue(graph, "Falconiere/comemory#999")).toThrow();
  });
});

describe("DryRunTest", () => {
  async function runDry(issue: string, extra: string[] = []) {
    const proc = Bun.spawn(
      ["bun", "run", LAUNCH, "--graph", FIXTURE, "--issue", issue, "--dry-run", ...extra],
      { stdout: "pipe", stderr: "pipe" },
    );
    const [stdout, stderr, code] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    return { stdout, stderr, code };
  }

  test("ready issue prints the herdr sequence without state", async () => {
    const out = await runDry("Falconiere/comemory#255");
    expect(out.code).toBe(0);
    const lines = out.stdout.split("\n");
    expect(lines.some((l) => l.startsWith("git -C ") && l.endsWith("fetch origin main"))).toBe(
      true,
    );
    expect(
      lines.some((l) => l.includes("herdr worktree create") && l.includes("--base origin/main")),
    ).toBe(true);
    expect(lines.some((l) => l.startsWith("herdr agent start comemory-255 --kind claude"))).toBe(
      true,
    );
    expect(lines.some((l) => l.startsWith("herdr agent prompt comemory-255"))).toBe(true);
  });

  test("missing checkout is cloned into clone_root", async () => {
    const graph = JSON.parse(readFileSync(FIXTURE, "utf8")) as Graph;
    expect(findIssue(graph, "Falconiere/homebrew-tap#1").checkout).toBeNull();
    const out = await runDry("Falconiere/homebrew-tap#1", ["--force"]);
    expect(out.code).toBe(0);
    const cloneTo = `${graph.clone_root}/homebrew-tap`;
    expect(out.stdout).toContain(`gh repo clone Falconiere/homebrew-tap ${cloneTo}`);
  });

  test("blocked issue is refused", async () => {
    const out = await runDry("Falconiere/comemory#257");
    expect(out.code).not.toBe(0);
    expect(out.stderr + out.stdout).toContain("blocked");
  });
});
