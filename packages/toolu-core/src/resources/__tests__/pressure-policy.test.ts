/** The policy.json pressure override over real isolated resource roots. */
import { expect, test } from "bun:test";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import {
  acquireLease,
  prepareAgentMigration,
  readResourceState,
  writeJsonAtomic,
} from "../resources.ts";
import { advancePressure } from "../pressure.ts";

/** A fresh, held pressure record: 60 s of samples with 3% available memory. */
function heldPressure() {
  const now = Date.now();
  const bad = { cpus: 10, load: 1, availableBytes: 1e9, totalBytes: 32e9, steal: null };
  const pressure = advancePressure(advancePressure(undefined, { ...bad, at: now - 60_000 }), {
    ...bad,
    at: now,
  });
  expect(pressure.held).toBe(true);
  return pressure;
}

async function holdResources(root: string): Promise<void> {
  await writeJsonAtomic(join(root, "state.json"), {
    ...readResourceState(root),
    pressure: heldPressure(),
  });
}

const agent = { type: "agent", key: "a", stateDir: "epic-a", host: "claude" } as const;

test.concurrent("held pressure refuses admission unless policy disables pressure", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1 });
  await holdResources(root);
  await expect(acquireLease(root, agent)).rejects.toThrow("resource hold");

  await writeJsonAtomic(join(root, "policy.json"), { maxAgents: 1, pressure: false });
  await acquireLease(root, agent);
  await expect(acquireLease(root, { ...agent, key: "b" })).rejects.toThrow(
    "agent capacity exhausted (1)",
  );
  expect(readResourceState(root).pressure?.held).toBe(true);
});

test.concurrent("held pressure refuses migration unless policy disables pressure", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { pressure: false });
  const source = await acquireLease(root, agent);
  await writeJsonAtomic(join(root, "policy.json"), {});
  await holdResources(root);
  const options = { hostCap: 1, epicCap: 2 };
  await expect(prepareAgentMigration(root, source.token, "codex", options)).rejects.toThrow(
    "resource hold",
  );

  await writeJsonAtomic(join(root, "policy.json"), { pressure: false });
  expect(await prepareAgentMigration(root, source.token, "codex", options)).toMatchObject({
    pendingHost: "codex",
    stage: "replacing",
  });
});

test.concurrent("a non-boolean pressure policy fails closed", async () => {
  using sb = createSandbox();
  const root = join(sb.root, "resources");
  await writeJsonAtomic(join(root, "policy.json"), { pressure: "off" });
  await expect(acquireLease(root, agent)).rejects.toThrow("invalid resource pressure policy");
});
