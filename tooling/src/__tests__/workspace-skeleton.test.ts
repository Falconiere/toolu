// Bun workspace skeleton (#208)
import { expect, test } from "bun:test";
import { existsSync, readFileSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../../..");
const RootPackage = z.object({
  workspaces: z.array(z.string()),
  scripts: z.record(z.string(), z.string()),
});

function readText(rel: string): string {
  return readFileSync(join(ROOT, rel), "utf8");
}

function isDir(rel: string): boolean {
  return existsSync(join(ROOT, rel)) && statSync(join(ROOT, rel)).isDirectory();
}

const rootPackage = RootPackage.parse(JSON.parse(readText("package.json")));

test.concurrent("workspaces list core, opencode, conformance, and cli packages", () => {
  const packages = [
    "packages/toolu-core",
    "tools/toolu-opencode",
    "tools/toolu-conformance",
    "tools/toolu-cli",
  ];
  for (const pkg of packages) {
    expect(rootPackage.workspaces).toContain(pkg);
    expect(existsSync(join(ROOT, pkg, "package.json"))).toBe(true);
    expect(isDir(`${pkg}/src`)).toBe(true);
  }
});

test.concurrent("CI workflow defines a typescript job running test:ts", () => {
  // Same walk the awk did: enter at `  typescript:`, leave at the next two-space key.
  let inJob = false;
  let foundJob = false;
  let foundRun = false;
  for (const line of readText(".github/workflows/tests.yml").split("\n")) {
    if (line.startsWith("  typescript:")) {
      inJob = true;
      foundJob = true;
    } else if (/^ {2}[a-z]/.test(line)) {
      inJob = false;
    } else if (inJob && line.includes("bun run test:ts")) {
      foundRun = true;
    }
  }
  expect({ foundJob, foundRun }).toEqual({ foundJob: true, foundRun: true });
});

test.concurrent("root test delegates to the complete Bun lane and keeps explicit legacy commands", () => {
  const scripts = rootPackage.scripts;
  expect(scripts["test"]).toBe("bun run test:ts");
  for (const gate of [
    "test:conventions",
    "test:unit",
    "test:portable-core",
    "test:gate-coverage",
    "check:plugin-bundles",
    "check:hooks-json",
    "test:workspace",
    "test:pack",
    "test:conformance",
    "test:context-budget",
    "benchmarks --tier deterministic",
    "bench:shell --assert",
  ]) {
    expect(scripts["test:ts"]).toContain(gate);
  }
  expect(scripts["lint:shell"]).toBe("bash tooling/shellcheck.sh");
  expect(scripts["test:shell"]).toBe("bash tooling/bats-run.sh");
});

test.concurrent("CI keeps functional required check names and runs the Bun lane", () => {
  const workflow = readText(".github/workflows/tests.yml");
  expect(workflow).toMatch(/  shellcheck:\n    name: shellcheck[\s\S]*?run: bun run lint:shell/);
  expect(workflow).toMatch(/  typescript:\n    name: typescript[\s\S]*?bun run test:ts/);
  expect(workflow).toMatch(
    /  bats:\n    name: bats \(plugins\)[\s\S]*?bash tooling\/bats-run\.sh plugins tooling packages tools/,
  );
  expect(readText(".github/workflows/toolu-review.yml")).toContain("  review:");
});

test.concurrent("release-only path filters still run bundle-only changes", () => {
  const workflow = readText(".github/workflows/tests.yml");
  const releaseOnly = workflow
    .split("    paths-ignore: &release-only\n")[1]
    ?.split("  pull_request:")[0];
  expect(releaseOnly).toBeDefined();
  const ignored = releaseOnly?.match(/^      - .+$/gm) ?? [];
  expect(ignored).toEqual([
    '      - "CHANGELOG.md"',
    '      - ".release-please-manifest.json"',
    '      - "package.json"',
    '      - "packages/*/package.json"',
    '      - "tools/*/package.json"',
    '      - "plugins/*/.claude-plugin/plugin.json"',
    '      - "plugins/*/.codex-plugin/plugin.json"',
  ]);
  expect(workflow).toContain("    paths-ignore: *release-only");
  expect(ignored.join("\n")).not.toContain("hooks/dist");
});

test.concurrent("contributor guidance names the new default and active legacy checks", () => {
  expect(readText("AGENTS.md")).toContain("`bun run test` runs the TypeScript gate");
  expect(readText("docs/testing.md")).toContain("`bun run test` runs the TypeScript gate");
  expect(readText("plugins/toolu-review/skills/review/SKILL.md")).not.toContain("missing bats");
});

test.concurrent("portable-core documents tools/toolu-conformance", () => {
  expect(readText("docs/portable-core.md")).toContain("tools/toolu-conformance");
});

test.concurrent("conventions adoption documents local TS CI commands", () => {
  expect(readText("docs/conventions-adoption.md")).toMatch(/test:ts|typescript/);
});

// Real-subprocess suites run concurrently and outrun bun's 5 s default per-test
// timeout on a loaded CI runner. bunfig has no timeout key, and a preload's
// setDefaultTimeout does not reach bun's serial multi-file mode, so every
// `bun test` a script runs carries the flag (docs/testing.md).
test.concurrent("every bun test script raises the per-test timeout", () => {
  const runs = Object.values(rootPackage.scripts).flatMap((script) =>
    script.split("&&").filter((part) => part.trim().startsWith("bun test")),
  );
  expect(runs.length).toBeGreaterThan(0);
  for (const part of runs) {
    expect(part.trim()).toStartWith("bun test --timeout 60000 ");
  }
});
