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

test.concurrent("CI workflow defines a gate job running test:ts", () => {
  // Same walk the awk did: enter at `  gate:`, leave at the next two-space key.
  // The required `typescript` status needs this job (#362).
  let inJob = false;
  let foundJob = false;
  let foundRun = false;
  for (const line of readText(".github/workflows/tests.yml").split("\n")) {
    if (line.startsWith("  gate:")) {
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

test.concurrent("root test delegates to the complete Bun-only lane", () => {
  const scripts = rootPackage.scripts;
  expect(scripts["test"]).toBe("bun run test:ts");
  for (const gate of [
    "test:conventions",
    "test:unit",
    "test:portable-core",
    "test:gate-coverage",
    "test:final-removal",
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
  expect(scripts["lint:shell"]).toBeUndefined();
  expect(scripts["test:shell"]).toBeUndefined();
  expect(scripts["test:shell:serial"]).toBeUndefined();
});

test.concurrent("CI runs the Bun lane without retired shell jobs", () => {
  const workflow = readText(".github/workflows/tests.yml");
  expect(workflow).toMatch(/  gate:\n    name: bun run test[\s\S]*?bun run test:ts/);
  expect(workflow).toMatch(
    /  typescript:\n    name: typescript\n    needs: \[changes, gate, opencode, docs, rust, rust-musl, rust-conformance\]/,
  );
  expect(workflow).not.toMatch(/^  shellcheck:/m);
  expect(workflow).not.toMatch(/^  bats:/m);
  expect(readText(".github/workflows/toolu-review.yml")).toContain("  review:");
});

// #458: no workflow-level path filter. The release-only skip is a job-level
// `if` fed by .github/ci-paths.json, and bundle-only changes still run the gate.
test.concurrent("release-only files skip jobs through the data file, not path filters", () => {
  for (const file of ["tests.yml", "toolu-review.yml"]) {
    const workflow = readText(`.github/workflows/${file}`);
    expect(workflow).not.toMatch(/^\s+paths(-ignore)?:/m);
  }
  const data = z
    .looseObject({ releaseOnly: z.looseObject({ paths: z.array(z.string()) }) })
    .parse(JSON.parse(readText(".github/ci-paths.json")));
  expect(data.releaseOnly.paths).toEqual([
    "CHANGELOG.md",
    ".release-please-manifest.json",
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "packages/*/package.json",
    "tools/*/package.json",
    "tools/*/npm/package.json",
    "plugins/*/.claude-plugin/plugin.json",
    "plugins/*/.codex-plugin/plugin.json",
  ]);
  expect(data.releaseOnly.paths.join("\n")).not.toContain("hooks/dist");
});

test.concurrent("AGENTS.md maps each CI job to its path group and the aggregate (#458)", () => {
  const agents = readText("AGENTS.md");
  for (const row of [
    /^\| `gate` \(`bun run test`\) \| `ts` \|/m,
    /^\| `opencode \(ubuntu-latest\)`, `opencode \(macos-latest\)` \| `opencode` \|/m,
    /^\| `docs` \| `docs` \| `bun run test:docs`/m,
    /^\| `review` \| `changed` \|/m,
    /^\| `typescript` \| aggregate, `if: always\(\)` \|/m,
  ]) {
    expect(agents).toMatch(row);
  }
  expect(agents).toContain("`.github/ci-paths.json`");
  expect(agents).toContain("No workflow-level path filter");
});

test.concurrent("contributor guidance names the Bun default", () => {
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
