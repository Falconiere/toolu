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

/** The files beyond manifests and hook bundles that a plugin's shipped code reads at runtime. */
function stagePluginExtras(plugin: string, source: string, target: string): void {
  if (plugin === "pr-babysit") {
    // babysit-dispatch-fix.js renders each fixer's brief from this template.
    const brief = join("skills", "babysit", "references", "fixer-brief.md");
    copy(join(source, brief), join(target, brief));
  }
  if (plugin === "toolu") {
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
  if (plugin === "epic-orchestrator") {
    for (const item of readdirSync(join(source, "scripts"), { withFileTypes: true })) {
      const from = join(source, "scripts", item.name);
      const to = join(target, "scripts", item.name);
      if (item.isFile() && item.name.endsWith(".ts")) copy(from, to);
      else if (item.isDirectory() && item.name !== "__tests__" && item.name !== "fixtures")
        copyDirectory(from, to, (name) => name.endsWith(".ts"));
    }
    const references = "skills/epic-orchestrator/references";
    copyDirectory(join(source, references), join(target, references), (name) =>
      name.endsWith(".md"),
    );
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
    stagePluginExtras(entry.name, source, target);
    count++;
  }
  return count;
}

if (import.meta.main) {
  const destination = resolve(process.env.BUNDLE_PLUGINS_DEST ?? join(packageRoot, "plugins"));
  const count = stagePlugins(join(repositoryRoot, "plugins"), destination);
  if (count < 16) throw new Error(`only ${count} plugin manifests staged`);
  // stderr: `npm pack --json` prints the prepack's stdout ahead of its JSON report.
  process.stderr.write(`bundle-plugins: ${count} plugins staged in ${destination}\n`);
}
