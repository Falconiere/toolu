/**
 * Builds registry fixture modules (#257) as real single-file ESM bundles, with
 * the flags `tooling/src/build-plugins.ts` passes to `bun build`.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { RegistryEvent } from "../registry-types.ts";

const SOURCE = join(import.meta.dir, "fixtures", "module-fixture.ts");

export type FixtureModule = {
  spec: string;
  name: string;
  event: RegistryEvent;
  behavior:
    | "allow"
    | "advisory"
    | "ask"
    | "deny"
    | "block"
    | "throw"
    | "reject"
    | "invalid"
    | "gate";
};

/** Bundle one fixture variant to `outFile`. */
export async function buildModule(outFile: string, fixture: FixtureModule): Promise<void> {
  const result = await Bun.build({
    entrypoints: [SOURCE],
    target: "bun",
    format: "esm",
    sourcemap: "none",
    define: {
      FIXTURE_SPEC: JSON.stringify(fixture.spec),
      FIXTURE_NAME: JSON.stringify(fixture.name),
      FIXTURE_EVENT: JSON.stringify(fixture.event),
      FIXTURE_BEHAVIOR: JSON.stringify(fixture.behavior),
    },
  });
  const [output] = result.outputs;
  if (!result.success || output === undefined) {
    throw new Error(`bundling ${SOURCE} failed:\n${result.logs.map(String).join("\n")}`);
  }
  mkdirSync(dirname(outFile), { recursive: true });
  writeFileSync(outFile, await output.text());
}
