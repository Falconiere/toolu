import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import {
  dispatchFix,
  fixerAgentName,
  fixerOutcome,
  fixerReportPath,
  isClaudeTrustPrompt,
} from "../babysit/fixer-dispatch.ts";
import { loadFixerConfig, routeFix } from "../babysit/fixer-route.ts";

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/items/review-items.json");
const answers = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/jev/fix-tiers.json");
const trust = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/herdr/claude-trust-prompt.txt",
);
const original = {
  TOOLU_CONFIG_DIR: process.env.TOOLU_CONFIG_DIR,
  TOOLU_PROJECT_DIR: process.env.TOOLU_PROJECT_DIR,
  PATH: process.env.PATH,
};
const temps: string[] = [];
afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
  for (const [key, value] of Object.entries(original)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

function sandbox(): string {
  const dir = mkdtempSync(join(tmpdir(), "pr-babysit-fixer-"));
  temps.push(dir);
  process.env.TOOLU_CONFIG_DIR = dir;
  process.env.TOOLU_PROJECT_DIR = dir;
  return dir;
}

test("config path discovery tolerates a missing git executable", () => {
  const dir = mkdtempSync(join(tmpdir(), "pr-babysit-no-git-"));
  temps.push(dir);
  const routeModule = join(root, "plugins/pr-babysit/hooks/src/babysit/fixer-route.ts");
  const script = join(dir, "config-paths.ts");
  writeFileSync(
    script,
    `import { configPaths } from ${JSON.stringify(routeModule)}; console.log(JSON.stringify(configPaths("codex")));`,
  );
  const result = spawnSync(process.execPath, [script], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PATH: dir, TOOLU_PROJECT_DIR: "", TOOLU_CONFIG_DIR: dir },
  });
  expect(result.status, result.stderr).toBe(0);
  expect(JSON.parse(result.stdout).project).toBe("");
});

test("route groups captured review items and Jev answers in priority order", () => {
  sandbox();
  writeFileSync(
    join(process.env.TOOLU_CONFIG_DIR!, "toolu.config.json"),
    JSON.stringify({ prBabysit: { dispatch: "inline" } }),
  );
  const heuristic = routeFix({
    itemsFile: fixture,
    host: "claude",
    noJev: true,
    now: "2026-09-28T12:00:00Z",
  });
  expect((heuristic.groups as { tier: string }[]).map((g) => g.tier)).toEqual([
    "critical",
    "complex",
    "standard",
    "trivial",
  ]);
  expect(heuristic.dispatch).toBe("inline");
  const scored = routeFix({
    itemsFile: fixture,
    host: "claude",
    answersFile: answers,
    now: "2026-09-28T12:00:00Z",
  });
  expect(scored.source).toBe("jev");
  expect(
    (scored.groups as { tier: string; items: string[] }[]).map((g) => [g.tier, g.items.length]),
  ).toEqual([
    ["complex", 2],
    ["trivial", 4],
  ]);
  const raised = routeFix({
    itemsFile: fixture,
    host: "claude",
    answersFile: answers,
    raise: ["PRRT_kwDOSzUwAc6d2Ypf"],
  });
  expect(
    (raised.items as { id: string; tier: string }[]).find((i) => i.id === "PRRT_kwDOSzUwAc6d2Ypf")
      ?.tier,
  ).toBe("standard");
});

test("route rejects a null review item with a structured input error", () => {
  const dir = sandbox();
  const itemsFile = join(dir, "items.json");
  writeFileSync(itemsFile, JSON.stringify({ round: 1, items: [null] }));
  expect(() => routeFix({ itemsFile, host: "claude", noJev: true })).toThrow(
    "every item needs an id",
  );
});

test("config merges project overrides and rejects unsafe routing values", () => {
  const dir = sandbox();
  const project = join(dir, ".claude");
  mkdirSync(project);
  writeFileSync(
    join(dir, "toolu.config.json"),
    JSON.stringify({ prBabysit: { hosts: ["codex"], unattended: false } }),
  );
  writeFileSync(
    join(project, "toolu.config.json"),
    JSON.stringify({ prBabysit: { hosts: ["cursor-agent"] } }),
  );
  const config = loadFixerConfig("claude");
  expect(config.hosts).toEqual(["cursor"]);
  expect(config.unattended).toBe(false);
  expect(config.routing.claude[3]?.model).toBe("opus");
  writeFileSync(
    join(project, "toolu.config.json"),
    JSON.stringify({ prBabysit: { routing: { codex: [{ model: "a b" }, {}, {}, {}] } } }),
  );
  expect(() => loadFixerConfig("claude")).toThrow("shell-unsafe");
  writeFileSync(join(project, "toolu.config.json"), JSON.stringify({ prBabysit: false }));
  expect(() => loadFixerConfig("claude")).toThrow("must be an object");
});

test("cooling hosts route inline with a reason and no host assignment", () => {
  const dir = sandbox();
  writeFileSync(
    join(dir, "toolu.config.json"),
    JSON.stringify({ prBabysit: { hosts: ["codex", "cursor"] } }),
  );
  const stateFile = join(dir, "state.json");
  writeFileSync(
    stateFile,
    JSON.stringify({
      hostCooldowns: {
        codex: { until: "2026-09-28T13:00:00Z" },
        cursor: { until: "2026-09-28T13:00:00Z" },
      },
    }),
  );
  const route = routeFix({
    itemsFile: fixture,
    host: "claude",
    stateFile,
    noJev: true,
    now: "2026-09-28T12:00:00Z",
  });
  expect(route.dispatch).toBe("inline");
  expect(route.note).toContain("cooling: codex, cursor");
  expect((route.groups as { host: string | null }[]).every((g) => g.host === null)).toBe(true);
});

test("dispatch dry run uses the captured items and keeps state byte identical", () => {
  const dir = sandbox();
  const stateFile = join(dir, "state.json");
  const planFile = join(dir, "plan.json");
  const before = JSON.stringify({
    version: 2,
    repo: "Falconiere/toolu",
    number: 165,
    slot: "falconiere-toolu-165",
  });
  writeFileSync(stateFile, before);
  const items = JSON.parse(readFileSync(fixture, "utf8"));
  const id = items.items[0].id;
  writeFileSync(
    planFile,
    JSON.stringify({
      dispatch: "herdr",
      unattended: true,
      groups: [
        { seq: 1, tier: "critical", host: "claude", model: "opus", effort: "xhigh", items: [id] },
      ],
    }),
  );
  const result = dispatchFix({
    sub: "start",
    stateFile,
    planFile,
    itemsFile: fixture,
    repoRoot: join(dir, "repo"),
    branch: "feat/fix",
    base: "main",
    dryRun: true,
  });
  expect(result.dryRun).toBe(true);
  const commands = result.commands as string[][];
  expect(commands.map((c) => c.slice(0, 3))).toEqual([
    ["git", "-C", join(dir, "repo")],
    ["git", "-C", join(dir, "repo")],
    ["git", "-C", join(dir, "repo")],
    ["herdr", "worktree", "create"],
    ["herdr", "agent", "start"],
    ["herdr", "agent", "prompt"],
  ]);
  expect(result.brief as string).toContain("Falconiere/toolu#165");
  expect(result.brief as string).toContain("babysit-fixer-report.js");
  expect(result.brief as string).toContain(id);
  expect(readFileSync(stateFile, "utf8")).toBe(before);
  expect(existsSync(`${stateFile}.lock`)).toBe(false);
});

test("fixer brief report commands handle an apostrophe in the state path", () => {
  const dir = join(sandbox(), "owner's run");
  mkdirSync(dir);
  const stateFile = join(dir, "state.json");
  const planFile = join(dir, "plan.json");
  writeFileSync(
    stateFile,
    JSON.stringify({
      version: 2,
      repo: "Falconiere/toolu",
      number: 165,
      slot: "falconiere-toolu-165",
    }),
  );
  const id = JSON.parse(readFileSync(fixture, "utf8")).items[0].id;
  writeFileSync(
    planFile,
    JSON.stringify({
      dispatch: "herdr",
      groups: [
        { seq: 1, tier: "critical", host: "claude", model: "opus", effort: "xhigh", items: [id] },
      ],
    }),
  );
  const result = dispatchFix({
    sub: "start",
    stateFile,
    planFile,
    itemsFile: fixture,
    repoRoot: join(dir, "repo"),
    branch: "feat/fix",
    base: "main",
    dryRun: true,
  });
  const brief = result.brief as string;
  const done = brief.match(/^  `(bun .* done --note "<one-line summary>")`$/m)?.[1];
  const failed = brief.match(/^  `(bun .* failed --note "<the reason>")`$/m)?.[1];
  expect(done).toBeDefined();
  expect(failed).toBeDefined();
  if (done === undefined || failed === undefined) throw new Error("brief lacks report commands");
  const reportFile = fixerReportPath(stateFile, 1, 1);
  for (const [command, placeholder, status] of [
    [done, '"<one-line summary>"', "done"],
    [failed, '"<the reason>"', "failed"],
  ] as const) {
    const run = spawnSync("sh", ["-c", command.replace(placeholder, "'fixed'")], {
      encoding: "utf8",
    });
    expect(run.status, `${run.stdout}\n${run.stderr}`).toBe(0);
    expect(JSON.parse(readFileSync(reportFile, "utf8"))).toMatchObject({ status, note: "fixed" });
  }
});

test("dispatch rejects a malformed plan group before touching a worktree", () => {
  const dir = sandbox();
  const stateFile = join(dir, "state.json");
  const planFile = join(dir, "plan.json");
  writeFileSync(
    stateFile,
    JSON.stringify({
      version: 2,
      repo: "Falconiere/toolu",
      number: 165,
      slot: "falconiere-toolu-165",
    }),
  );
  writeFileSync(planFile, JSON.stringify({ dispatch: "herdr", groups: [null] }));
  expect(() =>
    dispatchFix({
      sub: "start",
      stateFile,
      planFile,
      itemsFile: fixture,
      repoRoot: join(dir, "repo"),
      branch: "feat/fix",
      base: "main",
      dryRun: true,
    }),
  ).toThrow("every plan group needs a host and string item ids");
  expect(existsSync(join(dir, "repo"))).toBe(false);

  const id = JSON.parse(readFileSync(fixture, "utf8")).items[0].id;
  writeFileSync(
    planFile,
    JSON.stringify({
      dispatch: "herdr",
      groups: [
        {
          seq: 1,
          tier: "standard",
          host: "codex",
          model: "gpt-6-sol",
          effort: "medium",
          items: [id],
        },
        {
          seq: 3,
          tier: "standard",
          host: "codex",
          model: "gpt-6-sol",
          effort: "medium",
          items: [id],
        },
      ],
    }),
  );
  expect(() =>
    dispatchFix({
      sub: "start",
      stateFile,
      planFile,
      itemsFile: fixture,
      repoRoot: join(dir, "repo"),
      branch: "feat/fix",
      base: "main",
      dryRun: true,
    }),
  ).toThrow("plan groups need contiguous 1-based seq values");
  expect(existsSync(join(dir, "repo"))).toBe(false);
});

test("report outcome and trust detection use real captured text", () => {
  const dir = sandbox();
  const report = join(dir, "report.json");
  writeFileSync(report, JSON.stringify({ status: "done" }));
  expect(fixerOutcome(report, "rate limit")).toBe("done");
  writeFileSync(report, "{");
  expect(fixerOutcome(report, "rate limit 429")).toBe("host_limited");
  expect(fixerOutcome(report, "tests passed")).toBe("no_report");
  const pane = readFileSync(trust, "utf8");
  const path = "/Users/falconiere/.herdr/worktrees/probe/pb-probe";
  expect(isClaudeTrustPrompt(pane, path)).toBe(true);
  expect(isClaudeTrustPrompt(pane, `${path}-other`)).toBe(false);
  expect(
    isClaudeTrustPrompt(pane.replace("No, exit", "No, continue without these permissions"), path),
  ).toBe(false);
});

test("fixer names match POSIX cksum-derived names", () => {
  for (const slot of ["falconiere-toolu-165", "falconiere-comemory-42"]) {
    const sum = Number(spawnSync("cksum", { input: slot, encoding: "utf8" }).stdout.split(" ")[0]);
    expect(fixerAgentName(slot, 2, 1)).toBe(
      `pb-${(sum % 16777216).toString(16).padStart(6, "0")}-r2g1`,
    );
  }
});

test("bundled fixer commands resolve the shipped plugin template and write an atomic report", () => {
  const dir = sandbox();
  const stateFile = join(dir, "state.json");
  const planFile = join(dir, "plan.json");
  const reportFile = join(dir, "report.json");
  writeFileSync(
    stateFile,
    JSON.stringify({
      version: 2,
      repo: "Falconiere/toolu",
      number: 165,
      slot: "falconiere-toolu-165",
    }),
  );
  const id = JSON.parse(readFileSync(fixture, "utf8")).items[0].id;
  writeFileSync(
    planFile,
    JSON.stringify({
      dispatch: "herdr",
      groups: [
        { seq: 1, tier: "critical", host: "claude", model: "opus", effort: "xhigh", items: [id] },
      ],
    }),
  );
  const dist = join(root, "plugins/pr-babysit/hooks/dist");
  writeFileSync(
    join(dir, "toolu.config.json"),
    JSON.stringify({ prBabysit: { dispatch: "inline" } }),
  );
  const routed = spawnSync(
    process.execPath,
    [join(dist, "babysit-route-fix.js"), "--items", fixture, "--host", "claude", "--no-jev"],
    { encoding: "utf8", env: { ...process.env, TOOLU_CONFIG_DIR: dir, TOOLU_PROJECT_DIR: dir } },
  );
  expect(routed.status, `${routed.stdout}\n${routed.stderr}`).toBe(0);
  expect(JSON.parse(routed.stdout).dispatch).toBe("inline");
  const dry = spawnSync(
    process.execPath,
    [
      join(dist, "babysit-dispatch-fix.js"),
      "start",
      "--state-file",
      stateFile,
      "--plan",
      planFile,
      "--items",
      fixture,
      "--repo-root",
      join(dir, "repo"),
      "--branch",
      "feat/fix",
      "--base",
      "main",
      "--dry-run",
    ],
    { encoding: "utf8" },
  );
  expect(dry.status).toBe(0);
  expect(JSON.parse(dry.stdout).brief).toContain("babysit-fixer-report.js");
  const report = spawnSync(
    process.execPath,
    [join(dist, "babysit-fixer-report.js"), reportFile, "done", "--note", "fixed"],
    { encoding: "utf8" },
  );
  expect(report.status).toBe(0);
  expect(JSON.parse(readFileSync(reportFile, "utf8"))).toMatchObject({
    status: "done",
    note: "fixed",
  });
});
