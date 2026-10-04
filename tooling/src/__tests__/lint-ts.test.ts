import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real-data checks for tooling/src/lint-ts.ts (`bun run lint:ts`) with real oxlint.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/src/lint-ts.ts");
const OXLINT_PATH = `${resolve(ROOT, "node_modules/.bin")}:${process.env["PATH"] ?? ""}`;

test.concurrent("lint-ts fails, naming the directory, when a config has no lint target", async () => {
  using sb = createSandbox({
    files: {
      "tooling/.oxlintrc.json": "{}\n",
      "tooling/src/ok.ts": "export const ok = 1;\n",
      "plugins/stray/.oxlintrc.json": "{}\n",
    },
  });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LINT_TS_ROOT: sb.project, PATH: OXLINT_PATH },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stdout).toContain(`lint:ts: ${sb.path("tooling")}`);
  expect(res.stderr).toContain(`lint:ts: no lint target under ${sb.path("plugins/stray")}`);
});

test.concurrent("lint-ts skips node_modules and the OpenCode plugin mirror, and passes clean sources", async () => {
  using sb = createSandbox({
    files: {
      "packages/a/.oxlintrc.json": "{}\n",
      "packages/a/src/a.ts": "export const a = 1;\n",
      "packages/a/node_modules/dep/.oxlintrc.json": "{}\n",
      "tools/toolu-opencode/plugins/x/.oxlintrc.json": "{}\n",
    },
  });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LINT_TS_ROOT: sb.project, PATH: OXLINT_PATH },
  });
  expect(res.exitCode).toBe(0);
  // oxlint may add its own summary lines (it does when stdout is not a TTY).
  const linted = res.stdout.split("\n").filter((line) => line.startsWith("lint:ts: "));
  expect(linted).toEqual([`lint:ts: ${sb.path("packages/a")}`]);
});

test.concurrent("lint-ts lints every plugin tree under a collection config and names the rule", async () => {
  using sb = createSandbox({
    files: {
      "plugins/.oxlintrc.json": '{ "rules": { "typescript/no-explicit-any": "error" } }\n',
      "plugins/a/hooks/src/ok.ts": "export const ok = 1;\n",
      "plugins/b/scripts/bad.ts": "export const bad: any = 1;\n",
      "plugins/c/skills/setup/scripts/ok.ts": "export const fine = 1;\n",
    },
  });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LINT_TS_ROOT: sb.project, PATH: OXLINT_PATH },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stdout + res.stderr).toContain("no-explicit-any");
  expect(res.stdout + res.stderr).toContain("b/scripts/bad.ts");
});

test.concurrent("lint-ts leaves a plugin directory that is not a lint target alone", async () => {
  using sb = createSandbox({
    files: {
      "plugins/.oxlintrc.json": '{ "rules": { "typescript/no-explicit-any": "error" } }\n',
      "plugins/a/hooks/src/ok.ts": "export const ok = 1;\n",
      "plugins/a/assets/bad.ts": "export const bad: any = 1;\n",
    },
  });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LINT_TS_ROOT: sb.project, PATH: OXLINT_PATH },
  });
  expect({ exitCode: res.exitCode, output: res.stdout + res.stderr }).toEqual({
    exitCode: 0,
    output: res.stdout + res.stderr,
  });
});

test.concurrent("lint-ts lints scripts/ and contract/ beside src/, not only the first target", async () => {
  const config = '{ "rules": { "typescript/no-explicit-any": "error" } }\n';
  using sb = createSandbox({
    files: {
      "tools/a/.oxlintrc.json": config,
      "tools/a/src/ok.ts": "export const ok = 1;\n",
      "tools/a/scripts/bad.ts": "export const bad: any = 1;\n",
      "tools/b/.oxlintrc.json": config,
      "tools/b/src/ok.ts": "export const ok = 1;\n",
      "tools/b/contract/worse.ts": "export const worse: any = 2;\n",
    },
  });
  const res = await run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { LINT_TS_ROOT: sb.project, PATH: OXLINT_PATH },
  });
  expect(res.exitCode).toBe(1);
  expect(res.stdout + res.stderr).toContain("scripts/bad.ts");
  expect(res.stdout + res.stderr).toContain("contract/worse.ts");
});
