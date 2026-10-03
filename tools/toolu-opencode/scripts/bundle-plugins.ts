#!/usr/bin/env bun
/** Stage the bundle-only OpenCode plugin catalog for npm packing. */
import { copyFileSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";

const packageRoot = resolve(import.meta.dir, "..");
const repositoryRoot = resolve(packageRoot, "../..");

function copy(source: string, target: string): void {
  mkdirSync(dirname(target), { recursive: true });
  copyFileSync(source, target);
}

function copyDirectory(source: string, target: string, accept: (name: string) => boolean): void {
  if (!existsSync(source)) return;
  for (const entry of readdirSync(source, { withFileTypes: true })) {
    const from = join(source, entry.name);
    const to = join(target, entry.name);
    if (entry.isDirectory()) {
      copyDirectory(from, to, accept);
    } else if (entry.isFile() && accept(entry.name)) {
      copy(from, to);
    }
  }
}

export function stagePlugins(sourceDirectory: string, outputDirectory: string): number {
  const sourceRoot = resolve(sourceDirectory);
  const destination = resolve(outputDirectory);
  if (!existsSync(sourceRoot)) throw new Error(`plugin source missing: ${sourceRoot}`);
  if (
    destination === sourceRoot ||
    destination === repositoryRoot ||
    destination === packageRoot ||
    sourceRoot.startsWith(destination + sep) ||
    destination.startsWith(sourceRoot + sep)
  ) {
    throw new Error(`unsafe bundle destination: ${destination}`);
  }
  rmSync(destination, { recursive: true, force: true });
  let count = 0;
  for (const entry of readdirSync(sourceRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const source = join(sourceRoot, entry.name);
    const target = join(destination, entry.name);
    const manifest = join(source, ".claude-plugin", "plugin.json");
    if (!existsSync(manifest)) continue;
    copy(manifest, join(target, ".claude-plugin", "plugin.json"));
    const hooks = join(source, "hooks");
    const hooksJson = join(hooks, "hooks.json");
    if (existsSync(hooksJson)) copy(hooksJson, join(target, "hooks", "hooks.json"));
    copyDirectory(join(hooks, "dist"), join(target, "hooks", "dist"), (name) =>
      name.endsWith(".js"),
    );
    copyDirectory(join(hooks, "docs"), join(target, "hooks", "docs"), (name) =>
      name.endsWith(".md"),
    );
    if (entry.name === "toolu") {
      copyDirectory(
        join(source, "settings"),
        join(target, "settings"),
        (name) => !/\.(sh|bash|bats)$/.test(name),
      );
      for (const name of readdirSync(join(source, "scripts"))) {
        if (/^debug-[a-z]+\.ts$/.test(name)) {
          copy(join(source, "scripts", name), join(target, "scripts", name));
        }
      }
    }
    // The orchestrator skill runs these through $TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR,
    // and launch-issue.ts renders the worker brief from the references.
    if (entry.name === "epic-orchestrator") {
      for (const dir of ["scripts", "scripts/trackers"]) {
        for (const name of readdirSync(join(source, dir))) {
          if (name.endsWith(".ts")) copy(join(source, dir, name), join(target, dir, name));
        }
      }
      const references = "skills/epic-orchestrator/references";
      copyDirectory(join(source, references), join(target, references), (name) =>
        name.endsWith(".md"),
      );
    }
    count++;
  }
  return count;
}

if (import.meta.main) {
  const destination = resolve(process.env.BUNDLE_PLUGINS_DEST ?? join(packageRoot, "plugins"));
  const count = stagePlugins(join(repositoryRoot, "plugins"), destination);
  if (count < 16) throw new Error(`only ${count} plugin manifests staged`);
  process.stdout.write(`bundle-plugins: ${count} plugins staged in ${destination}\n`);
}
