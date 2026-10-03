// The committed configs themselves: the repo is reached, and the newly gated trees are not allowed out.
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { matches, trackedFiles } from "../reach-files.ts";
import { REACH_CONFIG } from "../reach-run.ts";
import { JscpdSchema, ReachConfigSchema, readJson } from "../reach-schema.ts";
import { BIN_PATH, ROOT } from "./reach-fixture.ts";

const NEWLY_GATED = ["plugins/", "tools/toolu-cli/src/", "tools/toolu-opencode/scripts/"];

const DUPLICATED = [
  "export function summarize(values: readonly number[]): string {",
  "  let total = 0;",
  "  let largest = Number.NEGATIVE_INFINITY;",
  "  let smallest = Number.POSITIVE_INFINITY;",
  "  for (const value of values) {",
  "    total += value;",
  "    if (value > largest) largest = value;",
  "    if (value < smallest) smallest = value;",
  "  }",
  "  const mean = values.length === 0 ? 0 : total / values.length;",
  "  const spread = values.length === 0 ? 0 : largest - smallest;",
  "  const parts = [`total=${String(total)}`, `mean=${String(mean)}`, `spread=${String(spread)}`];",
  "  if (values.length === 0) parts.push('empty');",
  "  return parts.join(' ');",
  "}",
  "",
].join("\n");

test("the repo passes the reach check with its committed configs", async () => {
  const res = await run([process.execPath, join(ROOT, "tooling/src/check-gate-reach.ts")], {
    cwd: ROOT,
    env: { GATE_REACH_ROOT: undefined, PATH: BIN_PATH },
  });
  expect({ exitCode: res.exitCode, stderr: res.stderr }).toEqual({ exitCode: 0, stderr: "" });
});

test("no allowance lets a non-test file of a newly gated tree out of a gate", () => {
  const config = readJson(ROOT, REACH_CONFIG, ReachConfigSchema);
  const gated = trackedFiles(ROOT, config.exclude).filter(
    (file) => NEWLY_GATED.some((tree) => file.startsWith(tree)) && !file.includes("/__tests__/"),
  );
  expect(gated.length).toBeGreaterThan(150);
  const allowedOut = gated.flatMap((file) =>
    config.allowances
      .filter(({ glob }) => matches(glob, file))
      .map(({ tool }) => `${tool} ${file}`),
  );
  expect(allowedOut).toEqual([]);
});

test("the repo's jscpd config rejects a function duplicated across two plugins", async () => {
  using sb = createSandbox();
  const config = readFileSync(join(ROOT, ".jscpd.json"), "utf8");
  sb.write(".jscpd.json", config);
  // jscpd resolves every configured path, so each one has to exist.
  for (const path of JscpdSchema.parse(JSON.parse(config)).path) sb.write(`${path}/.keep`, "");
  sb.write("plugins/alpha/hooks/src/summarize.ts", DUPLICATED);
  sb.write("plugins/beta/hooks/src/summarize.ts", DUPLICATED);
  const res = await run(["jscpd", "--config", ".jscpd.json"], {
    cwd: sb.project,
    env: { PATH: BIN_PATH },
  });
  expect(res.exitCode).not.toBe(0);
  expect(res.stdout + res.stderr).toContain("plugins/alpha/hooks/src/summarize.ts");
  expect(res.stdout + res.stderr).toContain("plugins/beta/hooks/src/summarize.ts");
});
