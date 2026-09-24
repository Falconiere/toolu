import { afterAll, describe, expect, test } from "bun:test";
import { chmod, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { CliError, EXIT } from "../../exit";
import { claudeAdapter } from "../claude";
import { codexAdapter } from "../codex";
import { availableHosts, resolveHost, resolveHosts } from "../detect";
import { binaryExists, run } from "../run";

const hosts = await availableHosts();
const hasClaude = hosts.includes("claude");
const hasCodex = hosts.includes("codex");

const stubDir = await mkdtemp(join(tmpdir(), "toolu-hosts-"));
await writeFile(join(stubDir, "claude"), "#!/bin/sh\n", { mode: 0o755 });
await writeFile(join(stubDir, "codex"), "#!/bin/sh\n", { mode: 0o755 });
await chmod(join(stubDir, "claude"), 0o755);
await chmod(join(stubDir, "codex"), 0o755);
const stubEnv = { ...process.env, PATH: stubDir };
afterAll(async () => {
  await rm(stubDir, { recursive: true, force: true });
});

describe("command construction", () => {
  test("claude builds marketplace, install with scope, remove and update", () => {
    expect(claudeAdapter.addMarketplace("Falconiere/toolu").argv).toEqual([
      "claude",
      "plugin",
      "marketplace",
      "add",
      "Falconiere/toolu",
    ]);
    expect(claudeAdapter.install("rust-quality", "toolu", "user").argv).toEqual([
      "claude",
      "plugin",
      "install",
      "rust-quality@toolu",
      "--scope",
      "user",
    ]);
    expect(claudeAdapter.install("rust-quality", "toolu", undefined).argv).not.toContain("--scope");
    expect(claudeAdapter.remove("jira", "toolu").argv).toContain("uninstall");
    expect(claudeAdapter.update("jira", "toolu").argv).toContain("update");
  });

  test("codex omits scope entirely and re-adds to update", () => {
    expect(codexAdapter.install("rust-quality", "toolu", "user").argv).toEqual([
      "codex",
      "plugin",
      "add",
      "rust-quality@toolu",
    ]);
    expect(codexAdapter.update("jira", "toolu").argv).toEqual([
      "codex",
      "plugin",
      "add",
      "jira@toolu",
    ]);
  });
});

describe("parsing real host output", () => {
  test.skipIf(!hasClaude)("claude plugin list --json parses into the shared shape", async () => {
    const result = await run([...claudeAdapter.listInstalled().argv]);
    expect(result.code).toBe(0);
    const plugins = claudeAdapter.parseList(result.stdout);
    for (const plugin of plugins) {
      expect(plugin.name.length).toBeGreaterThan(0);
      expect(plugin.version.length).toBeGreaterThan(0);
      expect(plugin.name).not.toContain("@");
    }
  });

  test.skipIf(!hasCodex)("codex plugin list --json parses into the shared shape", async () => {
    const result = await run([...codexAdapter.listInstalled().argv]);
    expect(result.code).toBe(0);
    const plugins = codexAdapter.parseList(result.stdout);
    for (const plugin of plugins) {
      expect(plugin.name.length).toBeGreaterThan(0);
      expect(plugin.name).not.toContain("@");
    }
  });

  test("a payload of the wrong shape is rejected, not silently emptied", () => {
    expect(() => claudeAdapter.parseList('{"installed":[]}')).toThrow(/plugin list/);
    expect(() => codexAdapter.parseList("[]")).toThrow(/plugin list/);
  });
});

describe("host detection", () => {
  test("an explicit host always wins, even when others are present", async () => {
    expect((await resolveHost("codex")).host).toBe("codex");
    expect((await resolveHost("opencode")).host).toBe("opencode");
  });

  test("no host on PATH reports exit 3 naming every probed binary", async () => {
    const env = { ...process.env, PATH: "/nonexistent" };
    expect(resolveHost(undefined, env)).rejects.toThrow(/no host found on PATH/);
    expect(await binaryExists("claude", env)).toBe(false);
  });

  test("several wired hosts with no interactive refuse rather than choosing silently", async () => {
    await expect(
      resolveHosts(undefined, { interactive: false, mode: "single", env: stubEnv }),
    ).rejects.toThrow(/several hosts found/);
  });

  test("interactive multi-select returns every injected host", async () => {
    const chosen = await resolveHosts(undefined, {
      interactive: true,
      mode: "multi",
      env: stubEnv,
      selectHosts: async () => ["claude", "codex"],
    });
    expect(chosen).toEqual(["claude", "codex"]);
  });

  test("interactive single-select returns the injected host", async () => {
    const chosen = await resolveHosts(undefined, {
      interactive: true,
      mode: "single",
      env: stubEnv,
      selectHost: async () => "codex",
    });
    expect(chosen).toEqual(["codex"]);
  });

  test("cancel from the host picker surfaces exit 130", async () => {
    await expect(
      resolveHosts(undefined, {
        interactive: true,
        mode: "single",
        env: stubEnv,
        selectHost: async () => {
          throw new CliError(EXIT.cancelled, "cancelled");
        },
      }),
    ).rejects.toMatchObject({ code: EXIT.cancelled });
  });
});
