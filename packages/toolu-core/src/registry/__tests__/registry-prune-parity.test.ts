/**
 * `pruneInactiveModules` vs bash `toolu_registry_prune_inactive` (#257, AC-4).
 * Twin Codex roots get the same modules and snapshot; after one prune each the
 * surviving `.sh` files match, and each `.js` follows its `.sh` twin.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { pruneInactiveModules } from "../registry-prune.ts";

const REGISTRY_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/registry.sh");
const DIRS = ["pre-tools.d", "post-tools.d", "other.d"];

const SNAPSHOTS: Record<string, string | undefined> = {
  ready: JSON.stringify({ version: 1, status: "ready", plugins: ["present@t"] }),
  indeterminate: JSON.stringify({ version: 1, status: "indeterminate", plugins: [] }),
  missing: undefined,
  malformed: "{}",
};

function seed(root: string, snapshot: string | undefined): void {
  for (const dir of DIRS) {
    const d = join(root, "toolu", dir);
    mkdirSync(join(d, "absent@t__dir.sh"), { recursive: true });
    for (const file of [
      "present@t__a.sh",
      "present@t__a.js",
      "absent@t__b.sh",
      "absent@t__b.js",
      "a b@t__space.sh",
      "a b@t__space.js",
      "nosep.sh",
      "__x.sh",
      ".absent@t__hidden.sh",
      "absent@t__c.txt",
    ]) {
      writeFileSync(join(d, file), "");
    }
    symlinkSync(join(d, "absent@t__b.sh"), join(d, "absent@t__link.sh"));
  }
  if (snapshot !== undefined) writeFileSync(join(root, "toolu", "codex-plugins.json"), snapshot);
}

function survivors(root: string): string[] {
  return DIRS.flatMap((dir) =>
    readdirSync(join(root, "toolu", dir)).map((file) => `${dir}/${file}`),
  ).toSorted();
}

for (const [label, snapshot] of Object.entries(SNAPSHOTS)) {
  for (const host of ["codex", "claude"] as const) {
    test.concurrent(`matches bash toolu_registry_prune_inactive: ${label} snapshot on ${host}`, async () => {
      using sb = createSandbox();
      const bashRoot = sb.path("bash");
      const tsRoot = sb.path("ts");
      seed(bashRoot, snapshot);
      seed(tsRoot, snapshot);
      const hostEnv = (root: string) =>
        host === "codex"
          ? { CODEX_HOME: root, PLUGIN_ROOT: sb.path("plugin") }
          : { CLAUDE_CONFIG_DIR: root };

      const before = survivors(bashRoot);
      const res = await run(
        ["bash", "-c", '. "$1"; toolu_registry_prune_inactive', "_", REGISTRY_SH],
        {
          env: { HOME: sb.home, ...hostEnv(bashRoot) },
        },
      );
      expect(res).toMatchObject({ exitCode: 0, stderr: "" });
      const removed = pruneInactiveModules({ env: { HOME: sb.home, ...hostEnv(tsRoot) } });

      const bash = survivors(bashRoot);
      const bashRemoved = before.filter((f) => !bash.includes(f));
      // TypeScript removes exactly what bash removed, plus each removed module's .js twin.
      const jsTwins = bashRemoved.map((f) => f.replace(/\.sh$/u, ".js"));
      expect(survivors(tsRoot)).toEqual(bash.filter((f) => !jsTwins.includes(f)));
      const pruning = host === "codex" && label === "ready";
      expect(removed.length > 0).toBe(pruning);
      if (pruning) {
        expect(removed.map((p) => p.slice(tsRoot.length + "/toolu/".length)).toSorted()).toEqual([
          "post-tools.d/a b@t__space.js",
          "post-tools.d/a b@t__space.sh",
          "post-tools.d/absent@t__b.js",
          "post-tools.d/absent@t__b.sh",
          "pre-tools.d/a b@t__space.js",
          "pre-tools.d/a b@t__space.sh",
          "pre-tools.d/absent@t__b.js",
          "pre-tools.d/absent@t__b.sh",
        ]);
      }
    });
  }
}
