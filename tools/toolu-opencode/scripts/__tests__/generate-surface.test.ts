import { cpSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "bun:test";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { TOOLU_PLUGIN_ROOT } from "../lib/constants.ts";
import { planSurface, readTree, treesEqual, writeSurface } from "../lib/emit.ts";
import { selectPluginsByEnabledNames } from "../../src/select/resolve.ts";
import { runGenerateSurface } from "../generate-surface.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../..");
}

function planDefault(outDir: string) {
  const root = repoRoot();
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["toolu"]);
  if (!selected.ok) {
    throw new Error(selected.reason);
  }
  return planSurface({ repoRoot: root, outDir, plugins: selected.plugins });
}

test("generate twice yields identical tree", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-"));
  const plan = planDefault(out);
  writeSurface(plan, out);
  const first = readTree(out);
  writeSurface(plan, out);
  const second = readTree(out);
  expect(treesEqual(first, second, out)).toEqual([]);
});

test("drift check fails when a source skill changes", () => {
  const root = repoRoot();
  const copyRoot = mkdtempSync(join(tmpBase, "toolu-surface-src-"));
  cpSync(join(root, "plugins"), join(copyRoot, "plugins"), { recursive: true });
  const outDir = mkdtempSync(join(tmpBase, "toolu-surface-out-"));
  const code = runGenerateSurface(["--repo", copyRoot, "--out", outDir]);
  expect(code).toBe(0);

  const skillPath = join(copyRoot, "plugins/delivery-flow/skills/delivery-flow/SKILL.md");
  writeFileSync(skillPath, `${readFileSync(skillPath, "utf8")}\n<!-- drift probe -->\n`);
  const drift = runGenerateSurface(["--repo", copyRoot, "--out", outDir, "--check"]);
  expect(drift).toBe(1);
  rmSync(copyRoot, { recursive: true, force: true });
  rmSync(outDir, { recursive: true, force: true });
});

test("toolu surface ids are unique", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-ids-"));
  const plan = planDefault(out);
  const ids: string[] = [];
  for (const plugin of plan.catalog.plugins) {
    for (const item of [...plugin.skills, ...plugin.agents, ...plugin.commands]) {
      ids.push(item.id);
    }
  }
  expect(new Set(ids).size).toBe(ids.length);
});

test("generated skill resources exclude colocated test files", () => {
  const root = repoRoot();
  const out = mkdtempSync(join(tmpBase, "toolu-surface-resources-"));
  const selected = selectPluginsByEnabledNames(join(root, "plugins"), ["delivery-flow"]);
  if (!selected.ok) throw new Error(selected.reason);
  const plan = planSurface({ repoRoot: root, outDir: out, plugins: selected.plugins });
  expect([...plan.files.keys()].some((path) => path.includes("/__tests__/"))).toBe(false);
});

test("commands rewrite CLAUDE_PLUGIN_ROOT to TOOLU_PLUGIN_ROOT", () => {
  const out = mkdtempSync(join(tmpBase, "toolu-surface-rewrite-"));
  const plan = planDefault(out);
  const command = plan.files.get(join(out, "commands", "toolu--commit.md"));
  expect(command).toBeDefined();
  expect(command?.includes(TOOLU_PLUGIN_ROOT)).toBe(true);
  expect(command?.includes("${CLAUDE_PLUGIN_ROOT}")).toBe(false);
});

test("committed generated tree passes drift check", () => {
  const root = repoRoot();
  const generated = join(root, "tools/toolu-opencode/generated");
  expect(existsSync(generated)).toBe(true);
  const code = runGenerateSurface(["--repo", root, "--check"]);
  expect(code).toBe(0);
});
