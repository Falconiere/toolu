/**
 * Import cost of `@toolu/core/detect` (#254 AC-6). The entry is bundled with
 * the flags the plugin pipeline uses (`bun build --target bun --format esm`).
 * Each fresh `bun` process first loads `node:fs`, `node:child_process` and
 * `node:path`, which every hook bundle already loads through the host and
 * config layers. It then times one `import()` of the bundle, which is the
 * cost detect adds to a hook. The p50 must stay under 2 ms, and the bundle
 * must carry neither unbash nor zod. An empty module and a cold import (no
 * builtins preloaded) are reported beside it. docs/detect.md records a run.
 */
import { expect, test } from "bun:test";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { percentile } from "@toolu/conformance/harness/timing";

const ENTRY = join(import.meta.dir, "../detect.ts");
const RUNS = 15;
const BUDGET_MS = 2;

test("importing the bundled detect entry costs under 2 ms and loads no parser or zod", async () => {
  using sb = createSandbox();
  const bundle = sb.path("detect.js");
  const built = await run(
    [
      process.execPath,
      "build",
      ENTRY,
      "--target",
      "bun",
      "--format",
      "esm",
      "--sourcemap=none",
      "--outfile",
      bundle,
    ],
    { env: { HOME: sb.home } },
  );
  expect(built.exitCode).toBe(0);
  const source = readFileSync(bundle, "utf8");
  expect(source).not.toContain("node_modules/unbash");
  expect(source).not.toContain("node_modules/zod");
  expect(source).toContain("function pushTargetRoot");

  writeFileSync(sb.path("empty.js"), "export const empty = true;\n");
  const probe = (name: string, target: string, preload: boolean) => {
    const path = sb.path(name);
    const builtins = preload
      ? 'import "node:fs";\nimport "node:child_process";\nimport "node:path";\n'
      : "";
    writeFileSync(
      path,
      `${builtins}const started = performance.now();
await import(${JSON.stringify(pathToFileURL(target).href)});
console.log(performance.now() - started);
`,
    );
    return path;
  };
  const probes = {
    detect: probe("detect.mjs", bundle, true),
    empty: probe("empty.mjs", sb.path("empty.js"), true),
    cold: probe("cold.mjs", bundle, false),
  };
  const samples: Record<keyof typeof probes, number[]> = { detect: [], empty: [], cold: [] };
  // Interleaved, so machine load hits every probe alike; two warm-up rounds.
  for (let i = 0; i < RUNS + 2; i++) {
    for (const name of ["detect", "empty", "cold"] as const) {
      const res = await run([process.execPath, probes[name]], {
        cwd: sb.root,
        env: { HOME: sb.home },
      });
      expect(res.exitCode).toBe(0);
      if (i >= 2) samples[name].push(Number(res.stdout.trim()));
    }
  }
  const p50 = (values: number[]) => Number(percentile(values, 50).toFixed(3));
  const report = {
    bundleKb: Number((statSync(bundle).size / 1024).toFixed(1)),
    importP50Ms: p50(samples.detect),
    importP95Ms: Number(percentile(samples.detect, 95).toFixed(3)),
    emptyModuleP50Ms: p50(samples.empty),
    coldImportP50Ms: p50(samples.cold),
  };
  console.log(`detect import cost: ${JSON.stringify(report)}`);
  expect(report.importP50Ms).toBeLessThan(BUDGET_MS);
}, 120_000);
