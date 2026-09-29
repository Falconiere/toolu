/**
 * Monorepo dispatch. A workspace root has a guardrails.workspace.json naming
 * its packages and no source tree of its own. The four repo-level checks run
 * at the root against the manifest; every listed package then runs as if it
 * were a single-repo project, its paths prefixed so violations read
 * `packages/database/src/foo.ts`. Config is a value per package, so nothing
 * leaks between packages and no re-exec is needed.
 *
 * Fails closed (exit 3) on: both documents at the root, an empty package
 * list, an absolute or `..` package path, a listed directory that is absent or
 * has no config, and an existing but unlisted package — an unvisited tree is
 * not a clean one.
 */
import { readdirSync } from "node:fs";
import type { Dirent } from "node:fs";
import { join } from "node:path";
import type { Options } from "./cli.ts";
import { WORKSPACE_FILE, loadManifest } from "./config.ts";
import type { WorkspaceManifest } from "./config.ts";
import { packageContext, runFiles, runRepo, shellPwd, unchangedTree } from "./package-run.ts";
import { ROOT_CHECKS, selected } from "./registry.ts";
import { GuardrailsFatal, Reporter, fatal, printFatal } from "./report.ts";
import { editedPath, readStdin, stopHookActive } from "./stdin.ts";
import { exists, isDir, isFile } from "./walk.ts";

const SCAN_PRUNED = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  "out",
  "coverage",
  ".wrangler",
  ".next",
]);

export function isWorkspace(cwd: string): boolean {
  const env = process.env;
  if ((env["GR_WS_CHILD"] ?? "") !== "" || (env["GR_CONFIG"] ?? "") !== "") return false;
  if (!isFile(cwd, WORKSPACE_FILE)) return false;
  // Ambiguous, and the oxlint plugin resolves guardrails.config.json from the cwd.
  if (isFile(cwd, "guardrails.config.json")) {
    fatal(
      `both guardrails.config.json and ${WORKSPACE_FILE} are present — a workspace root must have only the manifest, because the oxlint plugin resolves guardrails.config.json from the working directory and would lint every package against this one`,
    );
  }
  return true;
}

function loadWorkspace(cwd: string): WorkspaceManifest {
  const manifest = loadManifest(cwd);
  if (manifest.packages.length === 0) {
    fatal(
      `${WORKSPACE_FILE} lists no packages — a workspace that governs nothing passes every check while enforcing none`,
    );
  }
  for (const pkg of manifest.packages) {
    if (pkg.startsWith("/") || pkg.includes("..")) {
      fatal(
        `${WORKSPACE_FILE} lists package "${pkg}" — package paths must be repo-relative with no ".." segment`,
      );
    }
    if (!isDir(cwd, pkg)) {
      fatal(
        `${WORKSPACE_FILE} lists package "${pkg}" but no such directory exists — fix the path or drop the entry`,
      );
    }
    if (!isFile(cwd, `${pkg}/guardrails.config.json`)) {
      fatal(
        `package "${pkg}" has no guardrails.config.json — every workspace package carries its own; copy one from the stack kit`,
      );
    }
  }
  return manifest;
}

/** The listed package containing `path`; the longest match wins. */
function ownerOf(packages: readonly string[], path: string): string {
  return packages
    .filter((pkg) => path.startsWith(`${pkg}/`))
    .reduce((a, b) => (b.length > a.length ? b : a), "");
}

function unlisted(dir: string): never {
  fatal(
    `"${dir}" has a guardrails.config.json but is not listed in ${WORKSPACE_FILE} — add it to packages, or delete the config; an unlisted package is never checked and the gate still reports green`,
  );
}

/** Every guardrails.config.json below the root, pruning dependency and build trees. */
function configsBelow(cwd: string, rel = ""): string[] {
  let names: Dirent[];
  try {
    names = readdirSync(join(cwd, rel), { withFileTypes: true });
  } catch {
    return [];
  }
  const out: string[] = [];
  for (const entry of names) {
    const path = rel === "" ? entry.name : `${rel}/${entry.name}`;
    if (entry.isDirectory() && !SCAN_PRUNED.has(entry.name)) out.push(...configsBelow(cwd, path));
    else if (entry.name === "guardrails.config.json" && !entry.isDirectory()) out.push(path);
  }
  return out;
}

function requireListed(cwd: string, packages: readonly string[]): void {
  for (const found of configsBelow(cwd).toSorted()) {
    if (ownerOf(packages, found) === "") unlisted(found.slice(0, found.lastIndexOf("/")));
  }
}

/** An unowned path is a root file (fine) unless an ancestor holds a stray package config. */
function requireOwned(cwd: string, packages: readonly string[], paths: readonly string[]): void {
  for (const path of paths) {
    if (ownerOf(packages, path) !== "" || !path.includes("/")) continue;
    let dir = path.slice(0, path.lastIndexOf("/"));
    for (;;) {
      if (isFile(cwd, `${dir}/guardrails.config.json`)) unlisted(dir);
      if (!dir.includes("/")) break;
      dir = dir.slice(0, dir.lastIndexOf("/"));
    }
  }
}

/** 3 beats everything (a broken guard rail is never a clean run), then 2, then 1. */
function normalized(status: number): number {
  return status >= 0 && status <= 3 ? status : 3;
}

function worst(a: number, b: number): number {
  return Math.max(normalized(a), normalized(b));
}

/** One package as the re-exec'd child used to run it: its own config, repo or --file mode. */
function runPackage(cwd: string, pkg: string, paths: readonly string[] | null): number {
  try {
    const ctx = packageContext(join(cwd, pkg), `${pkg}/`, "guardrails.config.json");
    if (paths === null) runRepo(ctx, "");
    else runFiles(ctx, "", paths, true);
    return ctx.report.failed ? 1 : 0;
  } catch (err: unknown) {
    if (err instanceof GuardrailsFatal) printFatal(err.message);
    else
      printFatal(
        `package "${pkg}" failed unexpectedly: ${err instanceof Error ? err.message : String(err)}`,
      );
    return 3;
  }
}

function runRepoMode(cwd: string, manifest: WorkspaceManifest, only: string): number {
  requireListed(cwd, manifest.packages);
  const ctx = { root: cwd, config: manifest, report: new Reporter("") };
  for (const [id, check] of ROOT_CHECKS) if (selected(id, [], only)) check(ctx, "repo");
  let status = ctx.report.failed ? 1 : 0;
  // --only is not forwarded to packages, exactly as the bash dispatcher behaved.
  for (const pkg of manifest.packages) status = worst(status, runPackage(cwd, pkg, null));
  return status;
}

/** Bucket paths by owning package, package-relative; one run per bucket. */
function runPaths(cwd: string, manifest: WorkspaceManifest, paths: readonly string[]): number {
  let status = 0;
  for (const pkg of manifest.packages) {
    const bucket = paths
      .filter((p) => ownerOf(manifest.packages, p) === pkg)
      .map((p) => p.slice(pkg.length + 1));
    if (bucket.length > 0) status = worst(status, runPackage(cwd, pkg, bucket));
  }
  return status;
}

function hookMode(cwd: string, manifest: WorkspaceManifest): number {
  let edited = editedPath(readStdin());
  if (edited === "") return 0;
  const pwd = shellPwd(cwd);
  if (edited.startsWith(`${pwd}/`)) edited = edited.slice(pwd.length + 1);
  else if (edited.startsWith("/")) {
    fatal(
      `"${edited}" is outside the workspace root (${pwd}) — guardrails cannot place it in a package`,
    );
  }
  if (!exists(cwd, edited)) return 0;
  requireOwned(cwd, manifest.packages, [edited]);
  // A hook must exit 2, not 1: Claude Code ignores a 1 from a hook.
  const status = runPaths(cwd, manifest, [edited]);
  return status === 1 ? 2 : status;
}

export function runWorkspace(cwd: string, opts: Options): number {
  const manifest = loadWorkspace(cwd);
  if (opts.mode === "repo") return runRepoMode(cwd, manifest, opts.only);
  if (opts.mode === "file") {
    requireOwned(cwd, manifest.packages, opts.paths);
    return runPaths(cwd, manifest, opts.paths);
  }
  if (opts.mode === "hook") return hookMode(cwd, manifest);
  if (stopHookActive(readStdin()) || unchangedTree(cwd)) return 0;
  const status = runRepoMode(cwd, manifest, opts.only);
  return status === 1 ? 2 : status;
}
