/**
 * Startup test fixtures (#342): real plugin directories, either copied from
 * the repo catalog (so their committed bundles run) or written with the
 * generated launcher `hooks.json` and a small real bundle.
 */
import { cpSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { launcherHook } from "@toolu/core/launcher";
import { readPluginManifest } from "../../inventory/manifest.ts";
import type { PluginManifest } from "../../inventory/types.ts";

export const REPO_ROOT = resolve(import.meta.dir, "../../../../..");
export const PLUGINS_ROOT = join(REPO_ROOT, "plugins");

export type TempRoot = { readonly path: string; [Symbol.dispose](): void };

/** A disposable temp directory, real-pathed so it reads the way the child processes see it. */
export function tempRoot(prefix: string): TempRoot {
  const path = realpathSync(mkdtempSync(join(tmpdir(), prefix)));
  return { path, [Symbol.dispose]: () => rmSync(path, { recursive: true, force: true }) };
}

export function manifestOf(pluginDir: string): PluginManifest {
  const manifest = readPluginManifest(pluginDir);
  if (manifest === null) throw new Error(`no plugin manifest in ${pluginDir}`);
  return manifest;
}

/** The repo's own plugin, as committed. */
export function catalogPlugin(name: string): PluginManifest {
  return manifestOf(join(PLUGINS_ROOT, name));
}

/** A copy of a catalog plugin under `root`, so a test can break one of its bundles. */
export function copiedPlugin(root: string, name: string): PluginManifest {
  const dir = join(root, name);
  cpSync(join(PLUGINS_ROOT, name), dir, { recursive: true });
  return manifestOf(dir);
}

export type FixtureOptions = {
  /** Entry name → bundle source; each becomes a generated SessionStart launcher. */
  entries?: Record<string, string>;
  dependencies?: string[];
  matcher?: string;
  /** Raw `hooks.json` body, replacing the generated one. */
  hooksJson?: string;
};

/** A plugin named `name` under `root` with generated launcher hooks and real bundles. */
export function fixturePlugin(
  root: string,
  name: string,
  options: FixtureOptions = {},
): PluginManifest {
  const dir = join(root, name);
  mkdirSync(join(dir, ".claude-plugin"), { recursive: true });
  mkdirSync(join(dir, "hooks", "dist"), { recursive: true });
  const dependencies = (options.dependencies ?? []).map((dep) => ({
    name: dep,
    marketplace: "toolu",
  }));
  writeFileSync(
    join(dir, ".claude-plugin", "plugin.json"),
    JSON.stringify({ name, version: "1.0.0", dependencies }),
  );
  const entries = options.entries ?? {};
  const hooks = Object.keys(entries).map((entry) =>
    launcherHook({ plugin: name, event: "SessionStart", entry }),
  );
  const generated = {
    hooks: {
      SessionStart: [{ matcher: options.matcher ?? "startup|resume|clear|compact", hooks }],
    },
  };
  writeFileSync(
    join(dir, "hooks", "hooks.json"),
    options.hooksJson ?? JSON.stringify(generated, null, 2),
  );
  for (const [entry, source] of Object.entries(entries)) {
    writeFileSync(join(dir, "hooks", "dist", `${entry}.js`), source);
  }
  return manifestOf(dir);
}
