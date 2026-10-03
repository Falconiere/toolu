// check-gate-reach (`bun run check:gate-reach`) against sandbox git repos with real tool configs.
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { reachedConfigs, repoFiles, runReach } from "./reach-fixture.ts";
import type { RepoConfigs } from "./reach-fixture.ts";

const SCRIPTS_FILE = "packages/a/scripts/x.ts";

/** One way to take packages/a/scripts out of each tool's reach, and nothing else. */
const UNREACHED: ReadonlyArray<readonly [string, (configs: RepoConfigs) => void]> = [
  ["typecheck", (c) => (c.tsconfig = { include: ["packages/a/src/**/*.ts"] })],
  ["format", (c) => (c.formatCheck = "oxfmt --check packages/*/src")],
  ["oxlint", (c) => (c.oxlint = { ignorePatterns: ["scripts"] })],
  ["jscpd", (c) => (c.jscpd = { path: ["packages/a/src"] })],
  ["knip", (c) => (c.knip = { workspaces: { "packages/a": { project: ["src/**/*.ts"] } } })],
];

function notReached(stderr: string): string[] {
  return stderr.split("\n").filter((line) => line.includes("not reached by"));
}

test.concurrent("a repo every tool reaches passes", async () => {
  using sb = createSandbox({ git: true, files: repoFiles(reachedConfigs()) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

for (const [tool, unreach] of UNREACHED) {
  test.concurrent(`a tracked file outside ${tool} fails, naming the file and the tool`, async () => {
    const configs = reachedConfigs();
    unreach(configs);
    using sb = createSandbox({ git: true, files: repoFiles(configs) });
    const res = await runReach(sb);
    expect(res.exitCode).toBe(1);
    expect(notReached(res.stderr)).toEqual([`gate-reach: ${SCRIPTS_FILE}: not reached by ${tool}`]);
  });
}

test.concurrent("an allowance for the tool and file lets the unreached file through", async () => {
  const configs = reachedConfigs();
  configs.tsconfig = { include: ["packages/a/src/**/*.ts"] };
  configs.reach = {
    version: 1,
    exclude: [],
    allowances: [{ tool: "typecheck", glob: "packages/a/scripts/**", why: "legacy scripts" }],
  };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("an allowance for another tool does not cover the file", async () => {
  const configs = reachedConfigs();
  configs.tsconfig = { include: ["packages/a/src/**/*.ts"] };
  configs.reach = {
    version: 1,
    exclude: [],
    allowances: [{ tool: "format", glob: "packages/a/scripts/**", why: "wrong tool" }],
  };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain(`gate-reach: ${SCRIPTS_FILE}: not reached by typecheck`);
  expect(res.stderr).toContain(
    "gate-reach: stale allowance: format packages/a/scripts/** covers no unreached file",
  );
});

test.concurrent("an allowance that covers no unreached file is stale", async () => {
  const configs = reachedConfigs();
  configs.reach = {
    version: 1,
    exclude: [],
    allowances: [{ tool: "jscpd", glob: "packages/a/scripts/**", why: "no longer needed" }],
  };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr.trim()).toBe(
    "gate-reach: stale allowance: jscpd packages/a/scripts/** covers no unreached file",
  );
});

test.concurrent("an excluded tree is outside the file universe", async () => {
  const configs = reachedConfigs();
  configs.tsconfig = { include: ["packages/a/src/**/*.ts"] };
  configs.reach = { version: 1, exclude: ["packages/a/scripts/**"], allowances: [] };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("an exact-path jscpd or knip ignore is an exemption, not lost reach", async () => {
  const configs = reachedConfigs();
  configs.jscpd = {
    path: ["packages/a/src", "packages/a/scripts"],
    ignore: ["**/packages/a/src/a.ts"],
  };
  configs.knip = {
    workspaces: {
      "packages/a": { project: ["src/**/*.ts", "scripts/**/*.ts"], ignore: ["scripts/x.ts"] },
    },
  };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("an unknown key in gate-reach.json is misconfiguration", async () => {
  const configs = reachedConfigs();
  configs.reach = { version: 1, exclude: [], allowances: [], allowences: [] };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("gate-reach: tooling/gate-reach.json");
  expect(res.stderr).toContain("allowences");
});

test.concurrent("a tool config that is not JSON is misconfiguration, not an empty rule set", async () => {
  const files = repoFiles(reachedConfigs());
  files["knip.json"] = "{ not json";
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("gate-reach: knip.json");
});

test.concurrent("a format:check script that is not an oxfmt check is misconfiguration", async () => {
  const configs = reachedConfigs();
  configs.formatCheck = "prettier --check .";
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("format:check");
});

test.concurrent("outside a git work tree the check refuses to pass", async () => {
  using sb = createSandbox({ files: repoFiles(reachedConfigs()) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("gate-reach: git ls-files failed");
});

test.concurrent("a repo with no tracked TypeScript governs nothing and fails", async () => {
  const files = repoFiles(reachedConfigs());
  delete files["packages/a/src/a.ts"];
  delete files["packages/a/scripts/x.ts"];
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("no tracked TypeScript files");
});
