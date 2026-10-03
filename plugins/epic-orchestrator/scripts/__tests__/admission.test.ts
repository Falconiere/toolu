/** Saved pool corruption must fail before launch or resource mutation. */
import { expect, test } from "bun:test";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { readResourceState } from "@toolu/core/resources";
import { launchReservation } from "../admission.ts";

test.concurrent("invalid saved host pools never fall back to default capacity", async () => {
  using sb = createSandbox();
  const state = sb.path("state");
  const root = join(sb.root, "resources");
  for (const pool of [
    null,
    [],
    [{ kind: "codex", cap: 0 }],
    [
      { kind: "codex", cap: 1 },
      { kind: "codex", cap: 2 },
    ],
  ]) {
    sb.write("state/pool.json", JSON.stringify(pool));
    await expect(
      launchReservation(state, "issue-1", "codex", 3, { resource_root: root }),
    ).rejects.toThrow();
    expect(readResourceState(root).leases).toEqual([]);
  }
  sb.write("state/pool.json", "{");
  await expect(
    launchReservation(state, "issue-1", "codex", 3, { resource_root: root }),
  ).rejects.toThrow();
  expect(readResourceState(root).leases).toEqual([]);
});
