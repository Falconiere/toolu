// check-gate-reach (`bun run check:gate-reach`) against sandbox git repos with real tool configs.
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { reachedConfigs, repoFiles, runReach } from "./reach-fixture.ts";
import type { RepoConfigs } from "./reach-fixture.ts";

const SCRIPTS_FILE = "packages/a/scripts/x.ts";
const NEGATED = "is not supported; reach cannot be decided for it";

/** Ways to take packages/a/scripts out of one tool's reach, and nothing else. */
const UNREACHED: ReadonlyArray<readonly [string, string, (configs: RepoConfigs) => void]> = [
  ["typecheck", "typecheck", (c) => (c.tsconfig = { include: ["packages/a/src/**/*.ts"] })],
  ["format", "format", (c) => (c.formatCheck = "oxfmt --check packages/*/src")],
  ["oxlint by a bare name", "oxlint", (c) => (c.oxlint = { ignorePatterns: ["scripts"] })],
  ["oxlint by a directory pattern", "oxlint", (c) => (c.oxlint = { ignorePatterns: ["scripts/"] })],
  ["oxlint by an anchored pattern", "oxlint", (c) => (c.oxlint = { ignorePatterns: ["/scripts"] })],
  ["jscpd", "jscpd", (c) => (c.jscpd = { path: ["packages/a/src"] })],
  [
    "knip",
    "knip",
    (c) => (c.knip = { workspaces: { "packages/a": { project: ["src/**/*.ts"] } } }),
  ],
];

/** Configs the check must refuse, with the one stderr line it prints. */
const MISCONFIGURED: ReadonlyArray<readonly [string, (configs: RepoConfigs) => void, string]> = [
  [
    "a negated oxlint ignore pattern",
    (c) => (c.oxlint = { ignorePatterns: ["!scripts"] }),
    `gate-reach: packages/a/.oxlintrc.json: negated pattern "!scripts" ${NEGATED}`,
  ],
  [
    "a negated knip project glob that an earlier glob would hide",
    (c) =>
      (c.knip = {
        workspaces: {
          "packages/a": { project: ["src/**/*.ts", "scripts/**/*.ts", "!scripts/x.ts"] },
        },
      }),
    `gate-reach: knip.json: negated pattern "!scripts/x.ts" ${NEGATED}`,
  ],
  [
    "a bare path in the jscpd ignore list",
    (c) => (c.jscpd = { path: ["packages/a/src", "packages/a/scripts"], ignore: [SCRIPTS_FILE] }),
    `gate-reach: .jscpd.json: ignore entry "${SCRIPTS_FILE}" is a bare path, which jscpd does not honour; write "**/${SCRIPTS_FILE}"`,
  ],
  [
    "a format:check script that is not an oxfmt check",
    (c) => (c.formatCheck = "prettier --check ."),
    'gate-reach: package.json: format:check is not an "oxfmt --check <paths>" script',
  ],
];

test.concurrent("a repo every tool reaches passes", async () => {
  using sb = createSandbox({ git: true, files: repoFiles(reachedConfigs()) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

for (const [label, tool, unreach] of UNREACHED) {
  test.concurrent(`a tracked file outside ${label} fails, naming the file and the tool`, async () => {
    const configs = reachedConfigs();
    unreach(configs);
    using sb = createSandbox({ git: true, files: repoFiles(configs) });
    const res = await runReach(sb);
    expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
      exitCode: 1,
      stderr: `gate-reach: ${SCRIPTS_FILE}: not reached by ${tool}`,
    });
  });
}

const FMT_STEP = '"fmt" => cargo(root, &["fmt", "--all", "--check"]),';
const FMT_ABSENT =
  "gate-reach: package.json: format:check is absent; crates/xtask/src/gate.rs must run `cargo fmt --all --check`";

test.concurrent("a missing format:check is the xtask fmt step", async () => {
  const configs = reachedConfigs();
  delete configs.formatCheck;
  const files = repoFiles(configs);
  files["crates/xtask/src/gate.rs"] = `    ${FMT_STEP}\n`;
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("a missing format:check without the xtask fmt step is misconfiguration", async () => {
  const configs = reachedConfigs();
  delete configs.formatCheck;
  const files = repoFiles(configs);
  files["crates/xtask/src/gate.rs"] = "fn step() {}\n";
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr: FMT_ABSENT,
  });
});

for (const [label, misconfigure, line] of MISCONFIGURED) {
  test.concurrent(`${label} is misconfiguration`, async () => {
    const configs = reachedConfigs();
    misconfigure(configs);
    using sb = createSandbox({ git: true, files: repoFiles(configs) });
    const res = await runReach(sb);
    expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
      exitCode: 3,
      stderr: line,
    });
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
  expect(res.stderr.trim().split("\n")).toEqual([
    `gate-reach: ${SCRIPTS_FILE}: not reached by typecheck`,
    "gate-reach: stale allowance: format packages/a/scripts/** covers no unreached file",
  ]);
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
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr: "gate-reach: stale allowance: jscpd packages/a/scripts/** covers no unreached file",
  });
});

test.concurrent("an excluded tree is outside the file universe", async () => {
  const configs = reachedConfigs();
  configs.tsconfig = { include: ["packages/a/src/**/*.ts"] };
  configs.reach = { version: 1, exclude: ["packages/a/scripts/**"], allowances: [] };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("a per-file jscpd or knip exemption is not lost reach", async () => {
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

test.concurrent("a glob knip ignore entry does remove reach", async () => {
  const configs = reachedConfigs();
  configs.knip = {
    workspaces: {
      "packages/a": { project: ["src/**/*.ts", "scripts/**/*.ts"], ignore: ["**/x.ts"] },
    },
  };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr: `gate-reach: ${SCRIPTS_FILE}: not reached by knip`,
  });
});

test.concurrent("an unknown key in gate-reach.json is misconfiguration", async () => {
  const configs = reachedConfigs();
  configs.reach = { version: 1, exclude: [], allowances: [], allowences: [] };
  using sb = createSandbox({ git: true, files: repoFiles(configs) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr.trim()).toBe(
    'gate-reach: tooling/gate-reach.json: ✖ Unrecognized key: "allowences"',
  );
});

test.concurrent("a tool config that is not JSON is misconfiguration, not an empty rule set", async () => {
  const files = repoFiles(reachedConfigs());
  files["knip.json"] = "{ not json";
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toStartWith("gate-reach: knip.json: unreadable or not JSON (");
  expect(res.stderr.trim().split("\n")).toHaveLength(1);
});

test.concurrent("outside a git work tree the check refuses to pass", async () => {
  using sb = createSandbox({ files: repoFiles(reachedConfigs()) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toStartWith(`gate-reach: git ls-files failed in ${sb.project}: `);
});

test.concurrent("a repo with no tracked TypeScript governs nothing and fails", async () => {
  const files = repoFiles(reachedConfigs());
  delete files["packages/a/src/a.ts"];
  delete files["packages/a/scripts/x.ts"];
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr:
      "gate-reach: no tracked TypeScript files after exclude: a gate that governs nothing must not pass",
  });
});
