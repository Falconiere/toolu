/**
 * `registerModules` vs each real `register.sh` (#257, AC-3). Twin config roots
 * get the same foreign entries, stale entries of our own, old and fresh tmp
 * residue, a symlink, a dangling link and a directory. After one sync each,
 * the survivors match, except where TypeScript deliberately removes more:
 * our own stale `.js` and `.js` residue, and (for the quality plugins, whose
 * scripts only look at `post-tools.d`) our entries in `pre-tools.d`.
 * ast-grep left the list with its `register.sh` (#268); its register bundle
 * has its own suite in `plugins/ast-grep/hooks/src/__tests__`.
 * ts-quality left the list with its `register.sh` (#265); its register bundle
 * has its own suite in `plugins/ts-quality/hooks/src/__tests__`.
 */
import { expect, test } from "bun:test";
import {
  mkdirSync,
  readdirSync,
  readFileSync,
  symlinkSync,
  utimesSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { registerModules, type RegisterModuleSpec } from "../registry-register.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const BUNDLE = join(REPO, "plugins/toolu/hooks/dist/sample.js");
const DIRS = ["pre-tools.d", "post-tools.d"] as const;

type Plugin = {
  plugin: string;
  modules: Omit<RegisterModuleSpec, "bundle">[];
  /** Directories the bash script syncs and prunes. */
  bashDirs: readonly (typeof DIRS)[number][];
};

const PLUGINS: Plugin[] = [
  ...["python-quality", "rust-quality"].map((plugin) => ({
    plugin,
    modules: [{ name: plugin, event: "tool/post" as const }],
    bashDirs: ["post-tools.d"] as const,
  })),
];

function seed(root: string, spec: string): void {
  const now = Date.now();
  const at = (file: string, ageMs: number) => {
    writeFileSync(file, `${file}\n`);
    const when = new Date(now - ageMs);
    utimesSync(file, when, when);
  };
  for (const dir of DIRS) {
    const d = join(root, "toolu", dir);
    mkdirSync(join(d, `${spec}__dir.sh`), { recursive: true });
    at(join(d, "other@toolu__keep.sh"), 0);
    at(join(d, "other@toolu__keep.js"), 0);
    at(join(d, "other@toolu__x.sh.tmp.333"), 120_000);
    at(join(d, `${spec}__stale.sh`), 0);
    at(join(d, `${spec}__stale.js`), 0);
    at(join(d, `${spec}__old.sh.tmp.111`), 120_000);
    at(join(d, `${spec}__old.js.tmp.112`), 120_000);
    at(join(d, `${spec}__new.sh.tmp.222`), 10_000);
    symlinkSync(join(d, "other@toolu__keep.sh"), join(d, `${spec}__link.sh`));
    symlinkSync(join(d, "missing.sh"), join(d, `${spec}__dangle.sh`));
  }
}

function listing(root: string, dir: string): string[] {
  return readdirSync(join(root, "toolu", dir)).toSorted();
}

for (const p of PLUGINS) {
  test.concurrent(`matches ${p.plugin}/hooks/register.sh`, async () => {
    using sb = createSandbox();
    const spec = `${p.plugin}@toolu`;
    const bashRoot = sb.path("bash-cfg");
    const tsRoot = sb.path("ts-cfg");
    seed(bashRoot, spec);
    seed(tsRoot, spec);

    const script = join(REPO, "plugins", p.plugin, "hooks/register.sh");
    const res = await run(["bash", script], {
      env: { HOME: sb.home, TOOLU_CONFIG_DIR: bashRoot },
      stdin: "{}",
    });
    expect(res.exitCode).toBe(0);
    expect(res.stdout).toBe("");
    const modules = p.modules.map((m) => ({ ...m, bundle: BUNDLE }));
    const result = registerModules(spec, modules, {
      env: { HOME: sb.home, TOOLU_CONFIG_DIR: tsRoot },
    });
    expect(result.failed).toEqual([]);

    for (const dir of DIRS) {
      const current = p.modules
        .filter((m) => (m.event === "tool/pre") === (dir === "pre-tools.d"))
        .map((m) => `${spec}__${m.name}`);
      const extra = p.bashDirs.includes(dir)
        ? ["stale.js", "old.js.tmp.112"]
        : ["stale.sh", "stale.js", "old.sh.tmp.111", "old.js.tmp.112", "link.sh"];
      const bash = listing(bashRoot, dir).filter((f) => !current.includes(f.replace(/\.sh$/u, "")));
      const ts = listing(tsRoot, dir).filter((f) => !current.includes(f.replace(/\.js$/u, "")));
      expect(ts).toEqual(bash.filter((f) => !extra.some((e) => f === `${spec}__${e}`)));
      for (const name of current) {
        expect(
          readFileSync(join(tsRoot, "toolu", dir, `${name}.js`)).equals(readFileSync(BUNDLE)),
        ).toBe(true);
      }
    }
    // Foreign entries and a fresh in-flight tmp survive everywhere.
    for (const dir of DIRS) {
      expect(listing(tsRoot, dir)).toEqual(
        expect.arrayContaining([
          "other@toolu__keep.sh",
          "other@toolu__keep.js",
          "other@toolu__x.sh.tmp.333",
          `${spec}__new.sh.tmp.222`,
          `${spec}__dangle.sh`,
          `${spec}__dir.sh`,
        ]),
      );
    }
  });
}
