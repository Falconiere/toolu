/** Launcher tests: brief rendering and a real read-only dry run against a sandboxed copy of the #248 snapshot. */

import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { acquireLease, readResourceState, writeJsonAtomic } from "@toolu/core/resources";
import { findIssue, renderBrief } from "../launch-issue.ts";

const HERE = join(import.meta.dir, "..");
const FIXTURE = join(HERE, "fixtures", "epic248-graph.json");
const LAUNCH = join(HERE, "launch-issue.ts");

type Graph = Parameters<typeof findIssue>[0];

function loadGraph(): Graph {
  return JSON.parse(readFileSync(FIXTURE, "utf8")) as Graph;
}

test.concurrent("brief: every placeholder is filled", () => {
  const graph = loadGraph();
  const issue = findIssue(graph, "Falconiere/comemory#255");
  const paths = { worktree: "/wt/comemory/feat-255", status: "/state/status/comemory-255.json" };
  const brief = renderBrief(graph, issue, paths, "main");
  expect(brief).not.toContain("{{");
  expect(brief.startsWith("# Epic worker brief — Falconiere/comemory#255")).toBe(true);
  expect(brief).toContain("Closes Falconiere/comemory#255");
  expect(brief).toContain(
    "`gh issue view https://github.com/Falconiere/comemory/issues/255 --comments`",
  );
  expect(brief).toContain("Part of Falconiere/comemory#248");
  expect(brief).toContain(
    `| \`bun "${join(HERE, "report.ts")}" /state/status/comemory-255.json <phase>`,
  );
  expect(brief).not.toContain("report.sh");
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

test.concurrent("brief: Jira items read through jira.sh and resolve by key", () => {
  const graph = loadGraph();
  const jiraGraph = { ...graph, tracker: "jira" };
  const issue = { ...findIssue(graph, "Falconiere/comemory#255"), ref: "PAY-12", number: null };
  const brief = renderBrief(jiraGraph, issue, { worktree: "/wt", status: "/s.json" }, "main");
  expect(brief).toContain("Resolves PAY-12");
  expect(brief).toContain("jira.sh issue get PAY-12");
  expect(brief).not.toContain("{{");
});

test.concurrent("brief: find_issue by key or ref", () => {
  const graph = loadGraph();
  expect(findIssue(graph, "comemory-io-183").ref).toBe("CodaSignal/comemory.io#183");
  expect(() => findIssue(graph, "Falconiere/comemory#999")).toThrow();
});

/** Dry-run the launcher against a copy of the #248 graph whose state_dir is the
 * sandbox's, so no test reads a developer's real epic state (routes, records). */
async function runDry(sb: Sandbox, issue: string, extra: string[] = []) {
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: join(sb.root, "state") });
  const res = await run([
    "bun",
    "run",
    LAUNCH,
    "--graph",
    graph,
    "--issue",
    issue,
    "--dry-run",
    ...extra,
  ]);
  return { stdout: res.stdout, stderr: res.stderr, code: res.exitCode };
}

test.concurrent("dry run: ready issue prints the herdr sequence without state", async () => {
  using sb = createSandbox();
  const out = await runDry(sb, "Falconiere/comemory#255");
  expect(out.code).toBe(0);
  const lines = out.stdout.split("\n");
  expect(lines.some((l) => l.startsWith("git -C ") && l.endsWith("fetch origin main"))).toBe(true);
  expect(
    lines.some((l) => l.includes("herdr worktree create") && l.includes("--base origin/main")),
  ).toBe(true);
  expect(lines.some((l) => l.startsWith("herdr agent start comemory-255 --kind claude"))).toBe(
    true,
  );
  expect(lines.some((l) => l.startsWith("herdr agent prompt comemory-255"))).toBe(true);
});

test.concurrent("dry run: missing checkout is cloned into clone_root", async () => {
  using sb = createSandbox();
  const graph = loadGraph();
  expect(findIssue(graph, "Falconiere/homebrew-tap#1").checkout).toBeNull();
  const out = await runDry(sb, "Falconiere/homebrew-tap#1", ["--force"]);
  expect(out.code).toBe(0);
  const cloneTo = `${graph.clone_root}/homebrew-tap`;
  expect(out.stdout).toContain(`gh repo clone Falconiere/homebrew-tap ${cloneTo}`);
});

test.concurrent("dry run: routed host: codex with bypass, model, and effort; codex skill syntax", async () => {
  using sb = createSandbox();
  const out = await runDry(sb, "Falconiere/comemory#255", [
    "--kind",
    "codex",
    "--model",
    "gpt-6-sol",
    "--effort",
    "medium",
  ]);
  expect(out.code).toBe(0);
  const start = out.stdout.split("\n").find((l) => l.startsWith("herdr agent start"));
  expect(start).toBe(
    "herdr agent start comemory-255 --kind codex --pane '<root-pane>' --timeout 90000 -- " +
      "--no-daemon --dangerously-bypass-approvals-and-sandbox --model gpt-6-sol -c model_reasoning_effort=medium",
  );
  expect(out.stdout).toContain("`$delivery-flow:delivery-flow`");
  expect(out.stdout).not.toContain("`/delivery-flow:delivery-flow`");
});

test.concurrent("dry run: --safe keeps approval prompts on", async () => {
  using sb = createSandbox();
  const out = await runDry(sb, "Falconiere/comemory#255", ["--kind", "cursor-agent", "--safe"]);
  expect(out.code).toBe(0);
  expect(out.stdout).toContain("--kind cursor --pane '<root-pane>' --timeout 90000 -- --trust");
  expect(out.stdout).not.toContain("--yolo");
});

test.concurrent("dry run: blocked issue is refused", async () => {
  using sb = createSandbox();
  const out = await runDry(sb, "Falconiere/comemory#257");
  expect(out.code).not.toBe(0);
  expect(out.stderr + out.stdout).toContain("blocked");
});

test.concurrent("a persisted null host refuses even a forced launch before any mutation", async () => {
  using sb = createSandbox();
  mkdirSync(join(sb.root, "state/routes"), { recursive: true });
  writeFileSync(join(sb.root, "state/routes/comemory-255.json"), JSON.stringify({ host: null }));
  const out = await runDry(sb, "Falconiere/comemory#255", ["--force"]);
  expect(out.code).not.toBe(0);
  expect(out.stderr).toContain("no host capacity");
  expect(out.stdout).not.toContain("herdr agent start");
});

test.concurrent("malformed route and launch records refuse before acquiring ownership", async () => {
  using sb = createSandbox();
  const state = join(sb.root, "state");
  const root = join(sb.root, "resources");
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: state });
  for (const directory of ["routes", "issues"]) {
    const path = join(state, directory, "comemory-255.json");
    mkdirSync(join(state, directory), { recursive: true });
    const refusals: Record<string, string> =
      directory === "routes"
        ? {
            "{": "JSON Parse error",
            "[]": "expected object, received array",
            null: "expected object, received null",
          }
        : {
            "{": `invalid launch record ${path}: malformed JSON`,
            "[]": `invalid launch record ${path}: expected a JSON object`,
            null: `invalid launch record ${path}: expected a JSON object`,
          };
    for (const [source, refusal] of Object.entries(refusals)) {
      writeFileSync(path, source);
      const out = await run(
        [process.execPath, LAUNCH, "--graph", graph, "--issue", "comemory-255", "--force"],
        { env: { TOOLU_RESOURCE_HOME: root, PATH: "" } },
      );
      expect(out.exitCode).toBe(1);
      expect(out.stderr).toContain(refusal);
      expect(out.stderr).not.toContain("spawn git");
      expect(readResourceState(root).leases).toEqual([]);
    }
    writeFileSync(path, directory === "routes" ? '{"host":"codex"}' : "{}");
  }
});

test.concurrent("a forced actual launch still respects shared capacity before external work", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1 });
  await acquireLease(root, {
    type: "agent",
    key: "other",
    stateDir: "another-epic",
    host: "claude",
  });
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: join(sb.root, "state") });
  const out = await run(
    [process.execPath, LAUNCH, "--graph", graph, "--issue", "comemory-255", "--force"],
    { env: { TOOLU_RESOURCE_HOME: root, PATH: "" } },
  );
  expect(out.exitCode).toBe(1);
  expect(out.stderr).toContain("capacity exhausted");
  expect(readResourceState(root).leases).toHaveLength(1);
});

test.concurrent("a real missing dependency leaves durable uncertain ownership and refuses blind retry", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const state = join(sb.root, "state");
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: state });
  const argv = [process.execPath, LAUNCH, "--graph", graph, "--issue", "comemory-255", "--force"];
  const env = { TOOLU_RESOURCE_HOME: root, PATH: "" };
  const first = await run(argv, { env });
  const record = JSON.parse(readFileSync(join(state, "issues/comemory-255.json"), "utf8"));
  expect(record.lifecycle_error).toContain("gh");
  const reportedError = first.stderr.trim().startsWith("{")
    ? JSON.parse(first.stderr).error
    : first.stderr.trim();
  expect(reportedError).toContain(record.lifecycle_error);
  const nativeExit = record.native_error.exitCode;
  if (typeof nativeExit === "number") {
    expect(record.lifecycle_error).toContain("gh repo view Falconiere/comemory");
  }
  expect(first.exitCode).toBe(
    typeof nativeExit === "number" &&
      Number.isSafeInteger(nativeExit) &&
      nativeExit > 0 &&
      nativeExit <= 255
      ? nativeExit
      : 1,
  );
  expect(record.stage).toBe("uncertain");
  expect(record.attempt_id).toBe(readResourceState(root).leases[0]?.token);
  const retry = await run(argv, { env });
  expect(retry.stderr).toContain("previous launch/prompt outcome is uncertain");
  expect(readResourceState(root).leases).toHaveLength(1);
});

test.concurrent("incomplete lifecycle stages require explicit reconciliation before mutation", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const state = join(sb.root, "state");
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: state });
  mkdirSync(join(state, "issues"), { recursive: true });
  for (const stage of ["starting", "cleaning", "cleanup-incomplete", "replacing"]) {
    writeFileSync(join(state, "issues/comemory-255.json"), JSON.stringify({ stage }));
    const out = await run(
      [process.execPath, LAUNCH, "--graph", graph, "--issue", "comemory-255", "--force"],
      { env: { TOOLU_RESOURCE_HOME: root, PATH: "" } },
    );
    expect(out.exitCode).toBe(1);
    const recovery =
      stage === "starting" ? "--reprompt" : stage === "replacing" ? "--replace" : "finish-issue";
    expect(out.stderr).toContain(recovery);
    expect(readResourceState(root).leases).toEqual([]);
  }
});

test.concurrent("a legacy live-host change requires replace before acquiring ownership", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  const state = join(sb.root, "state");
  const graph = sb.write("graph.json", { ...loadGraph(), state_dir: state });
  mkdirSync(join(state, "issues"), { recursive: true });
  writeFileSync(
    join(state, "issues/comemory-255.json"),
    JSON.stringify({ stage: "running", kind: "claude", launches: 1 }),
  );
  const out = await run(
    [
      process.execPath,
      LAUNCH,
      "--graph",
      graph,
      "--issue",
      "comemory-255",
      "--kind",
      "codex",
      "--force",
    ],
    { env: { TOOLU_RESOURCE_HOME: root, PATH: "" } },
  );
  expect(out.exitCode).toBe(1);
  expect(out.stderr).toContain("host change requires explicit --replace");
  expect(readResourceState(root).leases).toEqual([]);
});
