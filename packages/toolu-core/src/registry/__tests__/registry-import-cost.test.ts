/**
 * Import cost per registry module (#257, AC-7). Twenty real bundles, each a
 * separate plugin's module inlining the state layer and zod, are run by
 * `runRegistry` in a fresh `bun` process. Reported: each module's import+run
 * time inside the process, the whole-process marginal cost per module (20
 * modules vs none), and, measured in the same run, what one bash registry
 * module costs the bash dispatcher (one `bash` spawn). docs/registry.md
 * records a run of this test.
 */
import { expect, test } from "bun:test";
import { mkdirSync, statSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { measureLatency, percentile } from "@toolu/conformance/harness/timing";
import { buildModule } from "./module-bundles.ts";

const RUNNER = join(import.meta.dir, "fixtures", "import-cost-runner.ts");
const COUNT = 20;
const RUNS = 15;

test("import cost per registry module is measured", async () => {
  using sb = createSandbox();
  const full = sb.path("full");
  const empty = sb.path("empty");
  const dir = join(full, "toolu", "post-tools.d");
  await Promise.all(
    Array.from({ length: COUNT }, (_, i) => {
      const spec = `m${String(i).padStart(2, "0")}@t`;
      return buildModule(join(dir, `${spec}__mod.js`), {
        spec,
        name: "mod",
        event: "tool/post",
        behavior: "advisory",
      });
    }),
  );
  mkdirSync(join(empty, "toolu", "post-tools.d"), { recursive: true });
  const bashModule = sb.path("module.sh");
  writeFileSync(bashModule, "exit 0\n");

  const probe = (root: string) =>
    run([process.execPath, RUNNER], { env: { HOME: sb.home, TOOLU_CONFIG_DIR: root } });
  const inProcess: number[] = [];
  const fullRuns = await measureLatency(
    async () => {
      const res = await probe(full);
      expect(res.exitCode).toBe(0);
      const parsed: { ms: number[]; statuses: string[] } = JSON.parse(res.stdout);
      expect(parsed.statuses).toEqual(Array.from({ length: COUNT }, () => "decision"));
      inProcess.push(...parsed.ms);
      return res;
    },
    { runs: RUNS, warmup: 2 },
  );
  const emptyRuns = await measureLatency(() => probe(empty), { runs: RUNS, warmup: 2 });
  const bashRuns = await measureLatency(() => run(["bash", bashModule]), { runs: RUNS, warmup: 2 });

  const bundleKb = statSync(join(dir, "m00@t__mod.js")).size / 1024;
  const report = {
    bundleKb: Number(bundleKb.toFixed(1)),
    inProcessP50: percentile(inProcess, 50),
    inProcessP95: percentile(inProcess, 95),
    marginalPerModule: (fullRuns.p50 - emptyRuns.p50) / COUNT,
    processP50: { none: emptyRuns.p50, twenty: fullRuns.p50 },
    bashModuleP50: bashRuns.p50,
  };
  console.log(`registry import cost: ${JSON.stringify(report)}`);
  // A sanity bound, not a budget: the recorded numbers are the deliverable.
  expect(report.inProcessP50).toBeLessThan(50);
}, 120_000);
