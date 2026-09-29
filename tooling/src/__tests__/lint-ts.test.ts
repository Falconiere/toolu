import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

// Real-data checks for tooling/src/lint-ts.ts (`bun run lint:ts`) with real oxlint.

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/src/lint-ts.ts");
const OXLINT_PATH = `${resolve(ROOT, "node_modules/.bin")}:${process.env["PATH"] ?? ""}`;

test.concurrent("lint-ts fails, naming the directory, when a config has neither src/ nor scripts/", async () => {
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
  expect(res.stderr).toContain(`lint:ts: no src/ or scripts/ under ${sb.path("plugins/stray")}`);
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
  expect(res.stdout.trim()).toBe(`lint:ts: ${sb.path("packages/a")}`);
});
