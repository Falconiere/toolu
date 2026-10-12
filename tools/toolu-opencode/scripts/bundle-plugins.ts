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
    // The fixer brief template `toolu babysit dispatch-fix` renders, beside its skill.
    const brief = join("skills", "babysit", "references", "fixer-brief.md");
    copy(join(source, brief), join(target, brief));
  }
  if (plugin === "jev") {
    // The one-line shim stays until #440. The hook publishes this path.
    copy(join(source, "scripts/jev.sh"), join(target, "scripts/jev.sh"));
  }
  if (plugin === "statusline") {
    // The native SessionStart publishes this plugin-owned compatibility shim.
    copy(join(source, "scripts/statusline.sh"), join(target, "scripts/statusline.sh"));
  }
  if (plugin === "toolu-review") {
    copy(join(source, "scripts/write-state.sh"), join(target, "scripts/write-state.sh"));
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
  // The orchestrator skill reads its references; `toolu epic` carries the brief template.
  if (plugin === "epic-orchestrator") {
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
  if (count < 12) throw new Error(`expected at least 12 plugin manifests, staged ${count}`);
  // stderr: `npm pack --json` prints the prepack's stdout ahead of its JSON report.
  process.stderr.write(`bundle-plugins: ${count} plugins staged in ${destination}\n`);
}
