/**
 * Sandbox state for the ast-grep golden cases (#268): which ast-grep modules
 * are registered (the bash ones from the base commit's `register.sh`, read out
 * of git, or `hooks/dist/register.js` behind its hooks.json launcher), and a
 * PATH on which ast-grep is absent. Everything else about a case is the same
 * for both registrations, so the module is the one variable.
 */
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, symlinkSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { launcherCommand } from "@toolu/core/launcher";
import {
  hostConfigRoot,
  installPlugins,
  pretoolEnv,
  type PretoolHost,
} from "@toolu/conformance/harness/pretool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";

export const PLUGIN_ROOT = resolve(import.meta.dir, "../../..");
export const REPO_ROOT = resolve(PLUGIN_ROOT, "../..");
/** The last commit whose ast-grep plugin is bash. */
export const BASH_BASE = "2912cd9d";
export const GOLDEN_PATH = join(import.meta.dir, "fixtures", "golden.json");

export type Registration =
  | { readonly kind: "bash"; readonly pluginRoot: string }
  | { readonly kind: "bundle" };

/** `git archive` the ast-grep plugin at `base` into `dir`; returns the extracted plugin root. */
export function extractBase(base: string, dir: string): string {
  const archive = spawnSync("git", ["-C", REPO_ROOT, "archive", base, "plugins/ast-grep"]);
  if (archive.status !== 0) throw new Error(`git archive ${base}: ${archive.stderr.toString()}`);
  const untar = spawnSync("tar", ["-x", "-C", dir], { input: archive.stdout });
  if (untar.status !== 0) throw new Error(`tar: ${untar.stderr.toString()}`);
  return join(dir, "plugins", "ast-grep");
}

/** Sandbox paths become `<ROOT>` so captures from different sandboxes compare. */
export function normalise(text: string, sb: Sandbox): string {
  return text.split(sb.root).join("<ROOT>");
}

/** Drop every registry entry of ast-grep, whoever wrote it. */
function clearAstGrep(sb: Sandbox, host: PretoolHost, extra: EnvPatch): void {
  const root = extra.TOOLU_CONFIG_DIR ?? hostConfigRoot(sb, host);
  for (const dir of ["pre-tools.d", "post-tools.d"]) {
    const path = join(root, "toolu", dir);
    if (!existsSync(path)) continue;
    for (const file of readdirSync(path).filter((f) => f.startsWith("ast-grep@toolu__"))) {
      rmSync(join(path, file), { force: true });
    }
  }
}

/** Register ast-grep's modules as `reg` says, replacing any registered before. */
export async function registerAstGrep(
  sb: Sandbox,
  host: PretoolHost,
  reg: Registration,
  extra: EnvPatch = {},
): Promise<void> {
  clearAstGrep(sb, host, extra);
  const plugin = reg.kind === "bash" ? reg.pluginRoot : PLUGIN_ROOT;
  const argv =
    reg.kind === "bash"
      ? ["bash", join(plugin, "hooks/register.sh")]
      : [
          "/bin/sh",
          "-c",
          launcherCommand({ plugin: "ast-grep", event: "SessionStart", entry: "register" }),
        ];
  const env = pretoolEnv(sb, host, {
    CLAUDE_PLUGIN_ROOT: plugin,
    PLUGIN_ROOT: host === "codex" ? plugin : undefined,
    ...extra,
  });
  const res = await run(argv, { cwd: sb.project, env, stdin: "{}" });
  if (res.exitCode !== 0 || res.stdout !== "") {
    throw new Error(`register ast-grep (${reg.kind}): ${String(res.exitCode)} ${res.stderr}`);
  }
}

/** A sandbox with toolu and ast-grep installed and no background git maintenance. */
export function baseSandbox(sb: Sandbox): void {
  sb.git("config", "maintenance.auto", "false");
  sb.git("config", "gc.auto", "0");
  installPlugins(sb, "toolu@toolu", "ast-grep@toolu");
}

const AST_GREP_BINARIES = new Set(["sg", "ast-grep"]);

/**
 * The current PATH with ast-grep taken off it: each directory holding `sg` or
 * `ast-grep` is replaced by a directory of links to its other entries, so every
 * other real tool stays reachable.
 */
export function pathWithoutAstGrep(sb: Sandbox): string {
  const dirs = (process.env.PATH ?? "").split(delimiter).filter((d) => d !== "");
  return dirs
    .map((dir, index) => {
      if (![...AST_GREP_BINARIES].some((name) => existsSync(join(dir, name)))) return dir;
      const farm = sb.path(`no-ast-grep/${String(index)}`);
      mkdirSync(farm, { recursive: true });
      for (const name of readdirSync(dir).filter((n) => !AST_GREP_BINARIES.has(n))) {
        symlinkSync(join(dir, name), join(farm, name));
      }
      return farm;
    })
    .join(delimiter);
}
