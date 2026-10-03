/** Routing: tiers, host capacity and cooldown, persisted routes, on the #248 snapshot. */

import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { acquireLease, prepareAgentMigration } from "@toolu/core/resources";
import {
  DEFAULT_TABLE,
  coolingHosts,
  heuristicTier,
  jevCommand,
  parseHosts,
  pickHost,
  routeIssues,
  tierFromScore,
} from "../route.ts";

const FIXTURE = join(import.meta.dir, "..", "fixtures", "epic248-graph.json");

const OPTS = { jev: false, reroute: false, table: DEFAULT_TABLE };

type G = Parameters<typeof routeIssues>[0];

/** The committed graph fixture with its state dir moved under `root`. */
function graphIn(root: string): G {
  const g = JSON.parse(readFileSync(FIXTURE, "utf8")) as G;
  g.state_dir = join(root, "state");
  return g;
}

test.concurrent("tiers: Jev score rounds with a small upward bias", () => {
  expect(tierFromScore(0)).toBe(0);
  expect(tierFromScore(1.36)).toBe(2);
  expect(tierFromScore(1.3)).toBe(1);
  expect(tierFromScore(3)).toBe(3);
});

test.concurrent("tiers: heuristic fallback", () => {
  expect(heuristicTier({ key: "a", ref: "a", title: "Fix typo in README" })).toBe(0);
  expect(heuristicTier({ key: "a", ref: "a", title: "Security hardening of tokens" })).toBe(3);
  expect(heuristicTier({ key: "a", ref: "a", title: "Add flag", unblocks: 4 })).toBe(2);
  expect(heuristicTier({ key: "a", ref: "a", title: "Add flag" })).toBe(1);
});

test.concurrent("host pool: parses caps", () => {
  expect(parseHosts("claude:2, codex, cursor-agent:1", 3)).toEqual([
    { kind: "claude", cap: 2 },
    { kind: "codex", cap: 3 },
    { kind: "cursor", cap: 1 },
  ]);
});

test.concurrent("host pool: most free capacity wins; cooling and full hosts are skipped", () => {
  const pool = parseHosts("claude:2,codex:2", 2);
  expect(pickHost(pool, {}, new Set())).toBe("claude");
  expect(pickHost(pool, { claude: 1 }, new Set())).toBe("codex");
  expect(pickHost(pool, {}, new Set(["claude"]))).toBe("codex");
  expect(pickHost(pool, { claude: 2, codex: 2 }, new Set())).toBeNull();
  expect(pickHost(pool, {}, new Set(), ["codex"])).toBe("codex");
});

test.concurrent("host pool: cooldowns expire", () => {
  const now = Date.parse("2026-09-26T12:00:00Z");
  const c = {
    codex: { until: "2026-09-26T13:00:00Z" },
    claude: { until: "2026-09-26T11:00:00Z" },
  };
  expect([...coolingHosts(c, now)]).toEqual(["codex"]);
});

test.concurrent("routeIssues: spreads a batch across hosts and persists each route", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const pool = parseHosts("claude:1,codex:1", 1);
  const { routes } = await routeIssues(
    g,
    ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
    pool,
    OPTS,
  );
  expect(routes.map((r) => r.host)).toEqual(["claude", "codex"]);
  for (const r of routes) {
    const saved = JSON.parse(readFileSync(join(g.state_dir, "routes", `${r.key}.json`), "utf8"));
    expect(saved.host).toBe(r.host);
    const tier =
      DEFAULT_TABLE.hosts[r.host ?? "claude"]?.[
        ["trivial", "standard", "complex", "critical"].indexOf(r.tier)
      ];
    expect(r.model).toBe(tier?.model ?? null);
  }
});

test.concurrent("routeIssues: a relaunch keeps its route; a cooling host forces a move", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const pool = parseHosts("claude:2,codex:2", 2);
  const first = (await routeIssues(g, ["Falconiere/comemory#255"], pool, OPTS)).routes[0];
  if (!first?.host) throw new Error("first route has no host");
  const again = (await routeIssues(g, ["Falconiere/comemory#255"], pool, OPTS)).routes[0];
  expect(again?.host).toBe(first.host);
  expect(again?.decided_at).toBe(first.decided_at);
  mkdirSync(g.state_dir, { recursive: true });
  writeFileSync(
    join(g.state_dir, "hosts.json"),
    JSON.stringify({ [first.host]: { until: "2999-01-01T00:00:00Z" } }),
  );
  const moved = (await routeIssues(g, ["Falconiere/comemory#255"], pool, OPTS)).routes[0];
  expect(moved?.host).not.toBe(first.host);
  expect(moved?.tier).toBe(first.tier);
});

test.concurrent("routeIssues: no capacity leaves the host empty instead of overbooking", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const pool = parseHosts("claude:1", 1);
  const { routes } = await routeIssues(
    g,
    ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
    pool,
    OPTS,
  );
  expect(routes.map((r) => r.host)).toEqual(["claude", null]);
});

test.concurrent("cached routes consume batch slots and revalidate reduced or removed pools", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const wanted = ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"];
  await routeIssues(g, wanted.slice(0, 1), parseHosts("claude:1", 1), OPTS);
  expect(
    (await routeIssues(g, wanted, parseHosts("claude:1", 1), OPTS)).routes.map((r) => r.host),
  ).toEqual(["claude", null]);
  expect(
    (await routeIssues(g, wanted, parseHosts("codex:1", 1), OPTS)).routes.map((r) => r.host),
  ).toEqual(["codex", null]);
});

test.concurrent("host pools reject nonpositive, fractional, nonfinite and duplicate caps", () => {
  for (const spec of [
    "claude:0",
    "claude:-1",
    "claude:1.5",
    "claude:NaN",
    "claude:Infinity",
    "claude:1,claude:2",
    "claude:1:2",
  ]) {
    expect(() => parseHosts(spec, 3)).toThrow();
  }
});

test.concurrent("routeIssues: a live record naming an unknown host neither crashes routing nor takes capacity", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  mkdirSync(join(g.state_dir, "issues"), { recursive: true });
  writeFileSync(
    join(g.state_dir, "issues", "x-1.json"),
    JSON.stringify({ stage: "running", kind: "gemini" }),
  );
  writeFileSync(
    join(g.state_dir, "issues", "x-2.json"),
    JSON.stringify({ stage: "running", kind: "claude" }),
  );
  const pool = parseHosts("claude:2", 2);
  const { routes } = await routeIssues(
    g,
    ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
    pool,
    OPTS,
  );
  // One claude slot is taken by x-2; the gemini record takes none.
  expect(routes.map((r) => r.host)).toEqual(["claude", null]);
});

test.concurrent("routeIssues: every durable ownership stage consumes real host capacity", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const stages = [
    "starting",
    "uncertain",
    "replacing",
    "running",
    "awaiting_merge",
    "cleaning",
    "cleanup-incomplete",
  ];
  mkdirSync(join(g.state_dir, "issues"), { recursive: true });
  for (const [index, stage] of stages.entries()) {
    writeFileSync(
      join(g.state_dir, "issues", `owned-${index}.json`),
      JSON.stringify({ stage, kind: "claude" }),
    );
  }
  for (const stage of ["merged", "abandoned", "unknown"]) {
    writeFileSync(
      join(g.state_dir, "issues", `released-${stage}.json`),
      JSON.stringify({ stage, kind: "claude" }),
    );
  }
  const { routes } = await routeIssues(
    g,
    ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
    parseHosts(`claude:${stages.length + 1}`, stages.length + 1),
    OPTS,
  );
  expect(routes.map((r) => r.host)).toEqual(["claude", null]);
});

test.concurrent("routing counts both source and reserved destination across epics", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const root = join(sb.root, "resources");
  const lease = await acquireLease(root, {
    type: "agent",
    key: "other-1",
    stateDir: "other-epic",
    host: "claude",
  });
  await prepareAgentMigration(root, lease.token, "codex", { hostCap: 1 });
  const { routes } = await routeIssues(g, ["comemory-255"], parseHosts("claude:1,codex:1", 2), {
    ...OPTS,
    resourceRoot: root,
  });
  expect(routes.map((route) => route.host)).toEqual([null]);
});

test.concurrent("routeIssues: issues resolve by ref or by key; unknown ones are refused", async () => {
  using sb = createSandbox();
  const g = graphIn(sb.root);
  const pool = parseHosts("claude:3", 3);
  const { routes } = await routeIssues(
    g,
    ["comemory-255", "CodaSignal/comemory.io#183"],
    pool,
    OPTS,
  );
  expect(routes.map((r) => r.ref)).toEqual([
    "Falconiere/comemory#255",
    "CodaSignal/comemory.io#183",
  ]);
  await expect(routeIssues(g, ["nope-1"], pool, OPTS)).rejects.toThrow(
    "nope-1 is not in the graph",
  );
});

async function runJev(argv: string[]): Promise<{ code: number; out: string }> {
  const proc = Bun.spawn([...argv, "ask"], { stdout: "pipe", stderr: "pipe" });
  const [out, code] = await Promise.all([new Response(proc.stdout).text(), proc.exited]);
  return { code, out };
}

test.concurrent("jevCommand: a jev.sh symlinked to a JS bundle runs under Bun, not bash", async () => {
  using sb = createSandbox();
  const bundle = join(sb.root, "jev.js");
  writeFileSync(bundle, '#!/usr/bin/env bun\nclass A {}\nconsole.log("bundle-ok");\n');
  chmodSync(bundle, 0o755);
  const link = join(sb.root, "jev.sh");
  symlinkSync(bundle, link);
  const argv = jevCommand(link);
  expect(argv).toEqual([process.execPath, link]);
  expect(await runJev(argv)).toEqual({ code: 0, out: "bundle-ok\n" });
  // The former invocation: bash parses the JS and fails.
  expect((await runJev(["bash", link])).code).not.toBe(0);
});

test.concurrent("jevCommand: a regular user override runs directly under its own shebang", async () => {
  using sb = createSandbox();
  const script = join(sb.root, "jev.sh");
  writeFileSync(script, "#!/bin/sh\necho override-ok\n");
  chmodSync(script, 0o755);
  const argv = jevCommand(script);
  expect(argv).toEqual([script]);
  expect(await runJev(argv)).toEqual({ code: 0, out: "override-ok\n" });
});
