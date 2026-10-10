/**
 * Fresh readiness (#342): with a stale readiness marker, an unrelated module
 * and an old copy of a selected plugin's module already on disk, a selected
 * plugin whose real startup cannot complete still leaves startup NotReady.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { PluginManifest } from "../../inventory/types.ts";
import { bootstrapRuntime } from "../runtime.ts";
import type { BootstrapResult } from "../result.ts";
import { REPO_ROOT, catalogPlugin, copiedPlugin, tempRoot } from "./fixtures.ts";

const OLD_MODULE = "toolu/post-tools.d/ts-quality@toolu__ts-quality.js";
const UNRELATED = "toolu/pre-tools.d/custom@local__extra.js";

/** A data root left by an earlier session: marker, unrelated module, old-version module. */
function staleDataRoot(root: string): string {
  const data = join(root, "data");
  const seeds: [string, string][] = [
    ["toolu/.session-start-ready", ""],
    [UNRELATED, "export default {};\n"],
    [OLD_MODULE, "// an older ts-quality\n"],
  ];
  for (const [path, body] of seeds) {
    mkdirSync(join(data, path, ".."), { recursive: true });
    writeFileSync(join(data, path), body);
  }
  return data;
}

function boot(root: string, plugins: PluginManifest[]): Promise<BootstrapResult> {
  mkdirSync(join(root, "project"), { recursive: true });
  return bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: join(root, "project"),
    dataRoot: staleDataRoot(root),
    plugins,
    isolatedHome: join(root, "home"),
  });
}

function reasonOf(result: BootstrapResult): string {
  if (result.status === "ready") throw new Error("expected not-ready");
  return result.reason;
}

test.concurrent("a partial registration is NotReady even though its other module was written", async () => {
  using root = tempRoot("toolu-ready-partial-");
  const astGrep = copiedPlugin(root.path, "ast-grep");
  const blocked = join(root.path, "data/toolu/post-tools.d/ast-grep@toolu__byte-savings.json");
  mkdirSync(join(root.path, "data/toolu/post-tools.d"), { recursive: true });
  mkdirSync(blocked);
  const reason = reasonOf(await boot(root.path, [astGrep]));
  expect(reason).toContain("ast-grep/register:");
  expect(reason).toContain("Is a directory");
  expect(
    existsSync(join(root.path, "data/toolu/pre-tools.d/ast-grep@toolu__search-nudge.json")),
  ).toBe(true);
});

test.concurrent("a missing helper source is NotReady", async () => {
  using root = tempRoot("toolu-ready-helper-");
  const jev = copiedPlugin(root.path, "jev");
  const source = join(jev.pluginDir, "scripts/jev.sh");
  rmSync(source);
  expect(reasonOf(await boot(root.path, [jev]))).toBe(
    `jev/session-start: helper ${source}: source-missing`,
  );
});

test.concurrent("a missing declared entry bundle is NotReady before the plugin runs", async () => {
  using root = tempRoot("toolu-ready-entry-");
  const tsQuality = copiedPlugin(root.path, "ts-quality");
  rmSync(join(tsQuality.pluginDir, "hooks/dist/check-toolu.js"));
  const result = await boot(root.path, [catalogPlugin("toolu"), tsQuality]);
  expect(reasonOf(result)).toBe("ts-quality: check-toolu: missing startup bundle");
  // Its register entry never ran: the old module is still the old one.
  expect(readFileSync(join(root.path, "data", OLD_MODULE), "utf8")).toBe(
    "// an older ts-quality\n",
  );
});

test.concurrent("the same stale data root with a healthy selection is ready from this run alone", async () => {
  using root = tempRoot("toolu-ready-healthy-");
  const result = await boot(root.path, [catalogPlugin("toolu"), catalogPlugin("ts-quality")]);
  if (result.status !== "ready") throw new Error(result.reason);
  const module = join(root.path, "data", OLD_MODULE);
  expect(
    readFileSync(module).equals(
      readFileSync(join(REPO_ROOT, "plugins/ts-quality/hooks/dist/post-tool-use.js")),
    ),
  ).toBe(true);
  expect(result.artifacts).toEqual([module]);
  expect(readFileSync(join(root.path, "data", UNRELATED), "utf8")).toBe("export default {};\n");
});
