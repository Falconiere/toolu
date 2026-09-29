/**
 * `@toolu/core/host` encoders against each host's documented output contract
 * (#252). For every host × event × decision, a real Bun process prints what
 * `encodeDecision` produced and exits with its code, exactly as a hook would;
 * the harness validates that output with the host's schema and reads its effect.
 */
import { describe, expect, test } from "bun:test";
import type { Decision } from "@toolu/core/decision";
import {
  HOST_EVENTS,
  HOST_NAMES,
  nativeEventName,
  supportsAsk,
  type HostEvent,
  type HostName,
} from "@toolu/core/host";
import {
  OpencodeEffectSchema,
  readHermesOutcome,
  readHostOutcome,
  readOpencodeOutcome,
  type Outcome,
} from "../hosts.ts";
import { run } from "../spawn.ts";

const HOOK = `
import { encodeDecision } from "@toolu/core/host";
const [host, event, decision] = JSON.parse(process.argv[1]);
const out = encodeDecision(host, event, decision);
if (out.kind === "effect") {
  process.stdout.write(JSON.stringify(out));
} else {
  process.stdout.write(out.stdout);
  process.stderr.write(out.stderr);
  process.exitCode = out.exitCode;
}
`;

const DECISIONS = {
  allow: { kind: "allow" },
  ask: { kind: "ask", reason: "confirm it" },
  deny: { kind: "deny", reason: "protected" },
  advisory: { kind: "advisory", message: "heads up" },
  post_block: { kind: "post_block", reason: "lint failed" },
  runtime_failure: { kind: "runtime_failure", reason: "gate crashed", code: "timeout" },
} satisfies Record<string, Decision>;

async function outcome(host: HostName, event: HostEvent, decision: Decision): Promise<Outcome> {
  const res = await run([process.execPath, "-e", HOOK, JSON.stringify([host, event, decision])], {
    cwd: import.meta.dir,
  });
  const native = nativeEventName(host, event) ?? "";
  if (host === "opencode") {
    expect(res.exitCode).toBe(0);
    const effect: unknown = JSON.parse(res.stdout);
    return readOpencodeOutcome(OpencodeEffectSchema.parse(effect));
  }
  return host === "hermes" ? readHermesOutcome(res) : readHostOutcome(host, native, res);
}

type Case = { host: HostName; event: HostEvent; name: string; decision: Decision };
const CASES: Case[] = HOST_NAMES.flatMap((host) =>
  HOST_EVENTS.filter((event) => nativeEventName(host, event) !== null).flatMap((event) =>
    Object.entries(DECISIONS).map(([name, decision]): Case => ({ host, event, name, decision })),
  ),
);

const PRE_ACTION = new Set<HostEvent>(["tool/pre", "shell/pre"]);

describe("every encoded output satisfies its host's contract", () => {
  for (const host of HOST_NAMES) {
    test.concurrent(`${host}: all events and decisions`, async () => {
      const mine = CASES.filter((c) => c.host === host);
      const results = await Promise.all(mine.map((c) => outcome(c.host, c.event, c.decision)));
      mine.forEach((c, i) => {
        const got = results[i];
        const label = `${c.host} ${c.event} ${c.name}`;
        expect(got, label).toBeDefined();
        if (got === undefined) return;
        if (c.name === "allow") expect(got.effect, label).toBe("allow");
        if (PRE_ACTION.has(c.event)) {
          const blocked =
            c.name === "deny" || c.name === "post_block" || c.name === "runtime_failure";
          if (blocked) expect(got.effect, label).toBe("deny");
          if (c.name === "ask") {
            expect(got.effect, label).toBe(supportsAsk(c.host, c.event) ? "ask" : "deny");
          }
          if (c.name === "advisory") expect(got.effect, label).toBe("allow");
        }
        if (got.effect === "ask") expect(supportsAsk(c.host, c.event), label).toBe(true);
      });
    });
  }
});

describe("host-specific effects", () => {
  test.concurrent("PermissionRequest denies on Claude and Codex and defers ask to the host prompt", async () => {
    const [claudeDeny, codexDeny, claudeAsk, codexAsk] = await Promise.all([
      outcome("claude", "permission/evaluate", DECISIONS.deny),
      outcome("codex", "permission/evaluate", DECISIONS.deny),
      outcome("claude", "permission/evaluate", DECISIONS.ask),
      outcome("codex", "permission/evaluate", DECISIONS.ask),
    ]);
    expect(claudeDeny).toEqual({ effect: "deny", reason: "protected" });
    expect(codexDeny).toEqual({ effect: "deny", reason: "protected" });
    expect(claudeAsk.effect).toBe("allow");
    expect(codexAsk.effect).toBe("allow");
  });

  test.concurrent("a prompt deny blocks on every host that can block prompts", async () => {
    const hosts = ["claude", "codex", "cursor", "opencode"] as const;
    const effects = await Promise.all(hosts.map((host) => outcome(host, "prompt", DECISIONS.deny)));
    expect(effects.map((o) => o.effect)).toEqual(["deny", "deny", "deny", "deny"]);
    expect(await outcome("hermes", "prompt", DECISIONS.deny)).toEqual({
      effect: "allow",
      context: "protected",
    });
  });

  test.concurrent("advice reaches the agent where the event has a context channel", async () => {
    const advice = DECISIONS.advisory;
    const got = await Promise.all([
      outcome("claude", "tool/pre", advice),
      outcome("codex", "session/start", advice),
      outcome("cursor", "tool/post", advice),
      outcome("hermes", "prompt", advice),
    ]);
    for (const o of got) expect(o).toEqual({ effect: "allow", context: "heads up" });
  });
});
