// check-legacy-exemptions (`bun run check:legacy-exemptions`) with real oxlint, jscpd and knip.
import { expect, test } from "bun:test";
import { chmodSync, readdirSync } from "node:fs";
import { resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import { BIN_PATH, ROOT } from "./reach-fixture.ts";

const SCRIPT = resolve(ROOT, "tooling/src/check-legacy-exemptions.ts");

const CLONE = [
  "export function tally(values: readonly number[]): string {",
  "  let total = 0;",
  "  let largest = Number.NEGATIVE_INFINITY;",
  "  for (const value of values) {",
  "    total += value;",
  "    if (value > largest) largest = value;",
  "  }",
  "  const mean = values.length === 0 ? 0 : total / values.length;",
  "  return [`total=${String(total)}`, `mean=${String(mean)}`, `max=${String(largest)}`].join(' ');",
  "}",
  "",
].join("\n");

function json(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

/** A tree where every exemption is still needed: a var, a clone and a dead export. */
function exemptedFiles(): Record<string, string> {
  return {
    "package.json": json({ name: "legacy-fixture", private: true }),
    "packages/a/.oxlintrc.json": json({
      rules: { "no-var": "error", "no-debugger": "error" },
      overrides: [
        { files: ["src/**/*.ts"], rules: { "no-console": "off" } },
        { files: ["src/legacy.ts"], rules: { "no-var": "off" } },
      ],
    }),
    "packages/a/src/legacy.ts": "export var legacy = 1;\n",
    "packages/a/src/clone-a.ts": CLONE,
    "packages/a/src/clone-b.ts": CLONE,
    ".jscpd.json": json({
      path: ["packages/a/src"],
      threshold: 0,
      exitCode: 1,
      minLines: 5,
      minTokens: 30,
      reporters: ["console"],
      ignore: ["**/__tests__/**", "**/packages/a/src/clone-b.ts"],
    }),
    "knip.json": json({
      workspaces: {
        ".": {
          entry: ["packages/k/src/main.ts"],
          project: ["packages/k/src/**/*.ts"],
          ignore: ["fixtures/**", "packages/k/src/dead.ts"],
        },
      },
    }),
    "packages/k/src/main.ts": 'import { used } from "./dead.ts";\n\nconsole.error(used);\n',
    "packages/k/src/dead.ts": "export const used = 1;\nexport const unused = 2;\n",
  };
}

function runLegacy(sb: Sandbox, path = BIN_PATH): Promise<RunResult> {
  return run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LEGACY_EXEMPTIONS_ROOT: sb.project, PATH: path },
    timeoutMs: 90_000,
  });
}

function liftedConfigs(sb: Sandbox): string[] {
  return readdirSync(sb.path("packages/a")).filter((name) => name.includes("lifted"));
}

test.concurrent("exemptions whose findings remain pass, and no lifted config is left behind", async () => {
  using sb = createSandbox({ files: exemptedFiles() });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
  expect(liftedConfigs(sb)).toEqual([]);
});

test.concurrent("an oxlint exemption for a rule the file no longer violates is stale", async () => {
  const files = exemptedFiles();
  files["packages/a/src/legacy.ts"] = "export const legacy = 1;\n";
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr:
      "legacy-exemptions: packages/a/.oxlintrc.json: src/legacy.ts no longer violates no-var; remove the exemption",
  });
});

test.concurrent("only the rule that stopped failing is reported for a file exempted twice", async () => {
  const files = exemptedFiles();
  files["packages/a/.oxlintrc.json"] = json({
    rules: { "no-var": "error", "no-debugger": "error" },
    overrides: [{ files: ["src/legacy.ts"], rules: { "no-var": "off", "no-debugger": "off" } }],
  });
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr:
      "legacy-exemptions: packages/a/.oxlintrc.json: src/legacy.ts no longer violates no-debugger; remove the exemption",
  });
});

test.concurrent("a jscpd exemption for a file with no clone is stale", async () => {
  const files = exemptedFiles();
  files["packages/a/src/clone-a.ts"] = "export const different = 1;\n";
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr:
      "legacy-exemptions: .jscpd.json: packages/a/src/clone-b.ts has no clone; remove the exemption",
  });
});

test.concurrent("a knip exemption for a file with no dead export is stale", async () => {
  const files = exemptedFiles();
  files["packages/k/src/dead.ts"] = "export const used = 1;\n";
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr:
      "legacy-exemptions: knip.json: packages/k/src/dead.ts has no dead export; remove the exemption",
  });
});

test.concurrent("an exemption for a file that is gone is stale in every tool", async () => {
  const files = exemptedFiles();
  delete files["packages/a/src/legacy.ts"];
  delete files["packages/a/src/clone-b.ts"];
  delete files["packages/k/src/dead.ts"];
  files["packages/k/src/main.ts"] = "console.error(1);\n";
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr.trim().split("\n")).toEqual([
    "legacy-exemptions: packages/a/.oxlintrc.json: src/legacy.ts does not exist; remove the exemption",
    "legacy-exemptions: .jscpd.json: packages/a/src/clone-b.ts does not exist; remove the exemption",
    "legacy-exemptions: knip.json: packages/k/src/dead.ts does not exist; remove the exemption",
  ]);
});

test.concurrent("a tool that crashes is misconfiguration, never taken for no findings", async () => {
  using sb = createSandbox({ files: exemptedFiles() });
  const broken = sb.write("bin/oxlint", "#!/bin/sh\necho 'oxlint: internal error' >&2\nexit 2\n");
  chmodSync(broken, 0o755);
  const res = await runLegacy(sb, `${sb.path("bin")}:${BIN_PATH}`);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr: "legacy-exemptions: oxlint exited 2: oxlint: internal error",
  });
  expect(liftedConfigs(sb)).toEqual([]);
});

test.concurrent("a report that is not JSON is misconfiguration", async () => {
  using sb = createSandbox({ files: exemptedFiles() });
  const broken = sb.write("bin/oxlint", "#!/bin/sh\necho 'not a report'\nexit 0\n");
  chmodSync(broken, 0o755);
  const res = await runLegacy(sb, `${sb.path("bin")}:${BIN_PATH}`);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr: "legacy-exemptions: oxlint report is not JSON",
  });
});

/** The fixture with its one oxlint exemption replaced by `overrides`. */
function withOverrides(overrides: object[]): Record<string, string> {
  const files = exemptedFiles();
  files["packages/a/.oxlintrc.json"] = json({ rules: { "no-var": "error" }, overrides });
  return files;
}

test.concurrent("an override with no rules is not an exemption, so nothing is asked of its file", async () => {
  const files = withOverrides([
    { files: ["src/gone.ts"], rules: {} },
    { files: ["src/legacy.ts"], rules: { "no-var": "off" } },
  ]);
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("a glob in an off-only override is a structural rule, not an exemption", async () => {
  const files = withOverrides([{ files: ["**/legacy.ts"], rules: { "no-var": "off" } }]);
  files["packages/a/src/legacy.ts"] = "export const legacy = 1;\n";
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("an off-only override mixing exact paths and globs is misconfiguration", async () => {
  const files = withOverrides([
    { files: ["src/legacy.ts", "src/**/*.gen.ts"], rules: { "no-var": "off" } },
  ]);
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr:
      "legacy-exemptions: packages/a/.oxlintrc.json: an override that only switches rules off mixes exact paths and globs (src/legacy.ts, src/**/*.gen.ts); split it so each exemption names its files exactly",
  });
});

test.concurrent("a bare path in the jscpd ignore list is misconfiguration: jscpd would not honour it", async () => {
  const files = exemptedFiles();
  files[".jscpd.json"] = json({
    path: ["packages/a/src"],
    ignore: ["**/__tests__/**", "packages/a/src/clone-b.ts"],
  });
  using sb = createSandbox({ files });
  const res = await runLegacy(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 3,
    stderr:
      'legacy-exemptions: .jscpd.json: ignore entry "packages/a/src/clone-b.ts" is a bare path, which jscpd does not honour; write "**/packages/a/src/clone-b.ts"',
  });
});

// A read-only directory does not stop root, so the case proves nothing there.
test.skipIf(process.getuid?.() === 0)(
  "a failure the check did not foresee is exit 3, never the exit 1 of a finding",
  async () => {
    using sb = createSandbox({ files: exemptedFiles() });
    chmodSync(sb.path("packages/a"), 0o555);
    try {
      const res = await runLegacy(sb);
      expect(res.exitCode).toBe(3);
      expect(res.stderr).toStartWith("legacy-exemptions: unexpected failure: ");
      expect(res.stderr).toContain("EACCES");
    } finally {
      chmodSync(sb.path("packages/a"), 0o755);
    }
  },
);

test("the repo's committed exemptions are all still needed", async () => {
  const res = await run([process.execPath, SCRIPT], {
    cwd: ROOT,
    env: { LEGACY_EXEMPTIONS_ROOT: undefined, PATH: BIN_PATH },
    timeoutMs: 110_000,
  });
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});
