/**
 * Packs a package the way `npm publish` does (#361): `npm pack` runs the
 * package's own `prepack` and applies its `files` field, and `--json` reports
 * each file's mode.
 *
 * `stageOpencode` copies `@toolu/opencode` into a temp repository layout, with
 * `plugins/` linked to the checkout's catalog. Its real prepack then stages
 * the catalog inside the copy, so packing never rewrites the working tree and
 * concurrent packs never share a staging directory.
 */
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, readdirSync, symlinkSync } from "node:fs";
import { join, resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../..");
const OPENCODE = "tools/toolu-opencode";
const PACK_TIMEOUT_MS = 180_000;
/** What `npm pack` produces from the package itself, never copied into a stage. */
const NOT_STAGED = new Set(["plugins", "node_modules"]);

export type PackedFile = { readonly path: string; readonly mode: number };

const PackReport = z.array(
  z.looseObject({
    filename: z.string(),
    files: z.array(z.looseObject({ path: z.string(), mode: z.number() })),
  }),
);

/** `npm pack --json` output; prepack may print before the JSON array, so parsing starts at its first line. */
export function parsePackReport(stdout: string): { filename: string; files: PackedFile[] } {
  const lines = stdout.split("\n");
  const start = lines.findIndex((line) => line.startsWith("["));
  if (start === -1) throw new Error(`npm pack printed no JSON report: ${stdout.slice(0, 500)}`);
  const [report] = PackReport.parse(JSON.parse(lines.slice(start).join("\n")));
  if (report === undefined) throw new Error("npm pack reported no package");
  return {
    filename: report.filename,
    files: report.files.map((file) => ({ path: file.path, mode: file.mode })),
  };
}

function npmPack(directory: string, args: readonly string[]): string {
  const result = spawnSync("npm", ["pack", "--json", ...args], {
    cwd: resolve(ROOT, directory),
    encoding: "utf8",
    timeout: PACK_TIMEOUT_MS,
  });
  if (result.error !== undefined)
    throw new Error(`npm pack in ${directory}: ${result.error.message}`);
  if (result.status !== 0) {
    throw new Error(`npm pack in ${directory} failed (exit ${result.status}): ${result.stderr}`);
  }
  return result.stdout;
}

/** The files `npm publish` would ship from `directory` (absolute, or relative to the repo root). */
export function packedFiles(directory: string): readonly PackedFile[] {
  return parsePackReport(npmPack(directory, ["--dry-run"])).files;
}

/** Pack `directory` into `destination`; the tarball's absolute path. */
export function packInto(directory: string, destination: string): string {
  const { filename } = parsePackReport(npmPack(directory, ["--pack-destination", destination]));
  return join(destination, filename);
}

/** A publishable copy of `@toolu/opencode` under `workDir`; returns the package directory. */
export function stageOpencode(workDir: string, root: string = ROOT): string {
  const source = join(root, OPENCODE);
  const stage = join(workDir, "repo", OPENCODE);
  mkdirSync(stage, { recursive: true });
  for (const entry of readdirSync(source)) {
    if (!NOT_STAGED.has(entry))
      cpSync(join(source, entry), join(stage, entry), { recursive: true });
  }
  symlinkSync(join(root, "plugins"), join(workDir, "repo", "plugins"));
  return stage;
}
