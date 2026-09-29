/**
 * Source of the registry modules the runner tests bundle (#257). `Bun.build`
 * bakes each variant's identity and behaviour in through `define`, so every
 * fixture is a real single-file ESM bundle like a plugin's `hooks/dist` output.
 * It imports the state layer the way a quality module does, which also makes
 * it a representative size for the import-cost measurement.
 *
 * Every variant first appends its name to `ctx.raw.marker` (when set), so a
 * test reads which modules ran, and in what order.
 */
import { appendFileSync } from "node:fs";
import type { Decision } from "../../../decision/decision.ts";
import { recordGateFailure } from "../../../state/state.ts";
import { defineRegistryModule, type RegistryEvent } from "../../registry-types.ts";

declare const FIXTURE_SPEC: string;
declare const FIXTURE_NAME: string;
declare const FIXTURE_EVENT: RegistryEvent;
declare const FIXTURE_BEHAVIOR: string;

const advisory = (): Decision => ({ kind: "advisory", message: `${FIXTURE_NAME} advises` });

const OUTCOMES: Readonly<Record<string, () => Decision>> = {
  allow: () => ({ kind: "allow" }),
  advisory,
  // Type-correct but rejected at runtime: an advisory needs a non-empty message.
  invalid: () => ({ kind: "advisory", message: "" }),
  ask: () => ({ kind: "ask", reason: `${FIXTURE_NAME} asks` }),
  deny: () => ({ kind: "deny", reason: `${FIXTURE_NAME} denies` }),
  block: () => ({ kind: "post_block", reason: `${FIXTURE_NAME} blocks` }),
};

function fail(): never {
  throw new Error(`${FIXTURE_NAME} exploded`);
}

export default defineRegistryModule({
  spec: FIXTURE_SPEC,
  name: FIXTURE_NAME,
  event: FIXTURE_EVENT,
  // Not async: the `throw` variant must throw synchronously, `reject` asynchronously.
  run(_event, ctx) {
    const { marker, gate } = ctx.raw;
    if (typeof marker === "string") appendFileSync(marker, `${FIXTURE_NAME}\n`);
    if (FIXTURE_BEHAVIOR === "throw") fail();
    if (FIXTURE_BEHAVIOR === "reject") return Promise.reject(new Error(`${FIXTURE_NAME} rejected`));
    if (FIXTURE_BEHAVIOR === "gate" && typeof gate === "string") {
      recordGateFailure(gate, "src/a.ts", FIXTURE_NAME, "violation", "- a.ts: bad\n", {
        env: ctx.env,
        host: ctx.host,
      });
    }
    const outcome = OUTCOMES[FIXTURE_BEHAVIOR];
    return Promise.resolve(outcome === undefined ? advisory() : outcome());
  },
});
