// ownedByLinter backing: a check a package hands to the linter must be a rule the linter runs.
import { expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { reachedConfigs, repoFiles, runReach } from "./reach-fixture.ts";

function workspaceFiles(owned: readonly string[], oxlint: object | null): Record<string, string> {
  const files = repoFiles(reachedConfigs());
  files["guardrails.workspace.json"] = JSON.stringify({ version: 2, packages: ["packages/a"] });
  files["packages/a/guardrails.config.json"] = JSON.stringify({ ownedByLinter: owned });
  if (oxlint === null) delete files["packages/a/.oxlintrc.json"];
  else files["packages/a/.oxlintrc.json"] = JSON.stringify(oxlint);
  return files;
}

const UNBACKED =
  'gate-reach: packages/a: ownedByLinter "no-barrels" has no rule at error level in packages/a/.oxlintrc.json';

test.concurrent("a package that owns a check with no lint config fails", async () => {
  using sb = createSandbox({ git: true, files: workspaceFiles(["no-barrels"], null) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain(UNBACKED);
});

test.concurrent("a lint config that does not enable the backing rule fails", async () => {
  using sb = createSandbox({ git: true, files: workspaceFiles(["no-barrels"], {}) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr: UNBACKED,
  });
});

test.concurrent("a backing rule below error level fails", async () => {
  const oxlint = { rules: { "house/no-barrels": ["warn", {}] } };
  using sb = createSandbox({ git: true, files: workspaceFiles(["no-barrels"], oxlint) });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr.trim() }).toEqual({
    exitCode: 1,
    stderr: UNBACKED,
  });
});

test.concurrent("an id outside the backing table fails closed", async () => {
  using sb = createSandbox({ git: true, files: workspaceFiles(["mystery"], {}) });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr.trim()).toBe(
    'gate-reach: packages/a: ownedByLinter "mystery" is not a check the linter can own',
  );
});

test.concurrent("a rule enabled through extends backs the check", async () => {
  const files = workspaceFiles(["no-barrels", "filename-case", "patterns"], {
    extends: ["../../lint/base.json"],
    rules: { "house/no-barrels": "error" },
  });
  files["lint/base.json"] = JSON.stringify({
    rules: { "unicorn/filename-case": ["error", { case: "kebabCase" }] },
  });
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test.concurrent("the package's own rule value wins over the extended one", async () => {
  const files = workspaceFiles(["filename-case"], {
    extends: ["../../lint/base.json"],
    rules: { "unicorn/filename-case": "off" },
  });
  files["lint/base.json"] = JSON.stringify({ rules: { "unicorn/filename-case": "error" } });
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain('ownedByLinter "filename-case" has no rule at error level');
});

test.concurrent("a listed package with no guardrails config is misconfiguration", async () => {
  const files = workspaceFiles([], {});
  delete files["packages/a/guardrails.config.json"];
  using sb = createSandbox({ git: true, files });
  const res = await runReach(sb);
  expect(res.exitCode).toBe(3);
  expect(res.stderr).toContain("gate-reach: packages/a/guardrails.config.json");
});
