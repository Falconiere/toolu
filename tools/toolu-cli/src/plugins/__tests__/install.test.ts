import { afterAll, describe, expect, test } from "bun:test";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { readMarketplace } from "../../catalog/manifest";
import { dependentsOf } from "../../catalog/order";
import { claudeAdapter } from "../../host/claude";
import { availableHosts } from "../../host/detect";
import { run } from "../../host/run";
import { installPlugins } from "../install";
import { listPlugins } from "../list";
import { removePlugins } from "../remove";

const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
const marketplace = await readMarketplace(resolve(REPO_ROOT, ".claude-plugin/marketplace.json"));
const hasClaude = (await availableHosts()).includes("claude");

const configDir = await mkdtemp(join(tmpdir(), "toolu-claude-"));
const env = { ...process.env, CLAUDE_CONFIG_DIR: configDir };
afterAll(async () => {
  await rm(configDir, { recursive: true, force: true });
});

const base = {
  adapter: claudeAdapter,
  marketplace,
  marketplaceName: "toolu",
  marketplaceSource: "Falconiere/toolu",
  scope: "user" as string | undefined,
  env,
};

describe("dry run plans without touching the host", () => {
  test("orders every dependent after the core plugin and runs nothing", async () => {
    const steps = await installPlugins({ ...base, requested: [], dryRun: true });
    expect(steps.length).toBe(marketplace.plugins.length);
    const core = steps.findIndex((step) => step.name === "toolu");
    for (const dependent of dependentsOf(marketplace, "toolu")) {
      expect(steps.findIndex((step) => step.name === dependent)).toBeGreaterThan(core);
    }
    for (const step of steps) {
      expect(step.outcome).toBe("skipped");
      expect(step.argv).toContain("install");
    }
    const after = await run([...claudeAdapter.listInstalled().argv], env);
    expect(claudeAdapter.parseList(after.stdout).length).toBe(0);
  });

  test("requesting a dependent plans its dependency first", async () => {
    const steps = await installPlugins({ ...base, requested: ["rust-quality"], dryRun: true });
    expect(steps.map((step) => step.name)).toEqual(["toolu", "rust-quality"]);
  });

  test("an unknown name is rejected before any command is built", async () => {
    expect(installPlugins({ ...base, requested: ["not-a-plugin"], dryRun: true })).rejects.toThrow(
      /unknown plugin/,
    );
  });
});

describe.skipIf(!hasClaude)("real install against a temporary CLAUDE_CONFIG_DIR", () => {
  test("installs a standalone plugin, reports it already installed, then removes it", async () => {
    const added = await run([...claudeAdapter.addMarketplace("Falconiere/toolu").argv], env);
    expect(added.code).toBe(0);

    const first = await installPlugins({ ...base, requested: ["jira"], dryRun: false });
    expect(first.map((step) => step.name)).toEqual(["jira"]);
    expect(first[0]?.outcome).toBe("installed");

    const state = await listPlugins(claudeAdapter, marketplace, env);
    expect(state.find((entry) => entry.name === "jira")?.installed).toBe(true);
    expect(state.find((entry) => entry.name === "rust-quality")?.installed).toBe(false);

    const second = await installPlugins({ ...base, requested: ["jira"], dryRun: false });
    expect(second[0]?.outcome).toBe("already");
    expect(second[0]?.detail).toMatch(/already installed at \d/);

    const removed = await removePlugins(claudeAdapter, "toolu", ["jira"], env);
    expect(removed[0]?.removed).toBe(true);
    const afterRemove = await listPlugins(claudeAdapter, marketplace, env);
    expect(afterRemove.find((entry) => entry.name === "jira")?.installed).toBe(false);
  }, 120_000);
});
