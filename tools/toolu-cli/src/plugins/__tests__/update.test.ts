import { describe, expect, test } from "bun:test";
import { claudeAdapter } from "../../host/claude";
import { availableHosts } from "../../host/detect";
import { updatePlugins } from "../update";

const hasClaude = (await availableHosts()).includes("claude");

describe.skipIf(!hasClaude)("updatePlugins against the real host", () => {
  test("reports plugins already at the offered version as current, running no update", async () => {
    const steps = await updatePlugins(claudeAdapter, "toolu", ["toolu"]);
    expect(steps.length).toBe(1);
    const step = steps[0];
    expect(step?.name).toBe("toolu");
    expect(["current", "updated", "failed"]).toContain(step?.outcome ?? "");
    if (step?.outcome === "current") expect(step.detail).toMatch(/^current at \d/);
  }, 60_000);

  test("an empty name list targets everything the host reports installed", async () => {
    const steps = await updatePlugins(claudeAdapter, "toolu", []);
    expect(steps.length).toBeGreaterThan(0);
    expect(steps.every((step) => step.argv.includes("update"))).toBe(true);
  }, 120_000);
});

describe("updatePlugins with an unreachable host binary", () => {
  test("a plugin the host does not report is still attempted and reported", async () => {
    const offline = { ...claudeAdapter, bin: "claude" };
    const steps = await updatePlugins(offline, "toolu", ["definitely-not-installed"], {
      ...process.env,
      PATH: "/nonexistent",
    });
    expect(steps.length).toBe(1);
    expect(steps[0]?.outcome).toBe("failed");
    expect(steps[0]?.detail).toBe("update failed");
  });
});
