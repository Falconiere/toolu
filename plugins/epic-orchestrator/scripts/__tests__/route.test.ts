/** Routing: tiers, host capacity and cooldown, persisted routes, on the #248 snapshot. */

import { describe, expect, test } from "bun:test";
import { mkdtempSync, readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  DEFAULT_TABLE,
  coolingHosts,
  heuristicTier,
  parseHosts,
  pickHost,
  routeIssues,
  tierFromScore,
} from "../route.ts";

const FIXTURE = join(import.meta.dir, "..", "fixtures", "epic248-graph.json");

type G = Parameters<typeof routeIssues>[0];

function graphInTmp(): G {
  const g = JSON.parse(readFileSync(FIXTURE, "utf8")) as G;
  g.state_dir = mkdtempSync(join(tmpdir(), "epic-route-"));
  return g;
}

describe("tiers", () => {
  test("Jev score rounds with a small upward bias", () => {
    expect(tierFromScore(0)).toBe(0);
    expect(tierFromScore(1.36)).toBe(2);
    expect(tierFromScore(1.3)).toBe(1);
    expect(tierFromScore(3)).toBe(3);
  });

  test("heuristic fallback", () => {
    expect(heuristicTier({ key: "a", ref: "a", title: "Fix typo in README" })).toBe(0);
    expect(heuristicTier({ key: "a", ref: "a", title: "Security hardening of tokens" })).toBe(3);
    expect(heuristicTier({ key: "a", ref: "a", title: "Add flag", unblocks: 4 })).toBe(2);
    expect(heuristicTier({ key: "a", ref: "a", title: "Add flag" })).toBe(1);
  });
});

describe("host pool", () => {
  test("parses caps", () => {
    expect(parseHosts("claude:2, codex, cursor-agent:1", 3)).toEqual([
      { kind: "claude", cap: 2 },
      { kind: "codex", cap: 3 },
      { kind: "cursor", cap: 1 },
    ]);
  });

  test("most free capacity wins; cooling and full hosts are skipped", () => {
    const pool = parseHosts("claude:2,codex:2", 2);
    expect(pickHost(pool, {}, new Set())).toBe("claude");
    expect(pickHost(pool, { claude: 1 }, new Set())).toBe("codex");
    expect(pickHost(pool, {}, new Set(["claude"]))).toBe("codex");
    expect(pickHost(pool, { claude: 2, codex: 2 }, new Set())).toBeNull();
    expect(pickHost(pool, {}, new Set(), ["codex"])).toBe("codex");
  });

  test("cooldowns expire", () => {
    const now = Date.parse("2026-09-26T12:00:00Z");
    const c = {
      codex: { until: "2026-09-26T13:00:00Z" },
      claude: { until: "2026-09-26T11:00:00Z" },
    };
    expect([...coolingHosts(c, now)]).toEqual(["codex"]);
  });
});

describe("routeIssues", () => {
  const opts = { jev: false, reroute: false, table: DEFAULT_TABLE };

  test("spreads a batch across hosts and persists each route", async () => {
    const g = graphInTmp();
    const pool = parseHosts("claude:1,codex:1", 1);
    const { routes } = await routeIssues(
      g,
      ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
      pool,
      opts,
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

  test("a relaunch keeps its route; a cooling host forces a move", async () => {
    const g = graphInTmp();
    const pool = parseHosts("claude:2,codex:2", 2);
    const first = (await routeIssues(g, ["Falconiere/comemory#255"], pool, opts)).routes[0];
    if (!first?.host) throw new Error("first route has no host");
    const again = (await routeIssues(g, ["Falconiere/comemory#255"], pool, opts)).routes[0];
    expect(again?.host).toBe(first.host);
    expect(again?.decided_at).toBe(first.decided_at);
    mkdirSync(g.state_dir, { recursive: true });
    writeFileSync(
      join(g.state_dir, "hosts.json"),
      JSON.stringify({ [first.host]: { until: "2999-01-01T00:00:00Z" } }),
    );
    const moved = (await routeIssues(g, ["Falconiere/comemory#255"], pool, opts)).routes[0];
    expect(moved?.host).not.toBe(first.host);
    expect(moved?.tier).toBe(first.tier);
  });

  test("no capacity leaves the host empty instead of overbooking", async () => {
    const g = graphInTmp();
    const pool = parseHosts("claude:1", 1);
    const { routes } = await routeIssues(
      g,
      ["Falconiere/comemory#255", "CodaSignal/comemory.io#183"],
      pool,
      opts,
    );
    expect(routes.map((r) => r.host)).toEqual(["claude", null]);
  });
});
