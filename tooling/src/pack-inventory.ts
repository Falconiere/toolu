/**
 * Asserts the published tarball's file list.
 *
 * The `toolu` CLI ships a Node bundle and its bundled marketplace manifest and
 * nothing else — not the TypeScript sources, and above all not the 3.2 MB bash
 * plugins/ tree, which Claude Code and Codex users never read from npm.
 */
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const CLI_DIR = resolve(import.meta.dir, "../../tools/toolu-cli");

const REQUIRED: readonly string[] = ["package.json", "assets/marketplace.json", "dist/cli.js"];
const FORBIDDEN: readonly string[] = ["plugins/", "src/", "node_modules/", ".env"];

/** File paths `bun pm pack --dry-run` reports for a package directory. */
export function packedFiles(directory: string): readonly string[] {
  const result = spawnSync("bun", ["pm", "pack", "--dry-run"], {
    cwd: directory,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`bun pm pack --dry-run failed in ${directory}: ${result.stderr}`);
  }
  return result.stdout
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line.startsWith("packed "))
    .map((line) => line.replace(/^packed \S+ /, ""));
}

function report(problems: readonly string[]): number {
  for (const problem of problems) process.stderr.write(`RED  ${problem}\n`);
  if (problems.length === 0) process.stdout.write("pack-inventory: ok\n");
  return problems.length === 0 ? 0 : 1;
}

function check(files: readonly string[]): readonly string[] {
  const problems: string[] = [];
  for (const required of REQUIRED) {
    if (!files.includes(required)) problems.push(`toolu tarball is missing ${required}`);
  }
  for (const forbidden of FORBIDDEN) {
    const leaked = files.filter((file) => file.startsWith(forbidden));
    for (const file of leaked) problems.push(`toolu tarball must not contain ${file}`);
  }
  const allowed = new Set(REQUIRED);
  const extra = files.filter((file) => !allowed.has(file));
  for (const file of extra) problems.push(`toolu tarball has an undeclared file: ${file}`);
  return problems;
}

process.exitCode = report(check(packedFiles(CLI_DIR)));
