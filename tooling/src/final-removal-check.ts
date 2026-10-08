/** Structural gate for the final Bun-only cutover (#279). */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { z } from "zod";

const ROOT = resolve(import.meta.dir, "../..");
const Row = z.object({
  id: z.string(),
  classification: z.string(),
  hostMechanism: z.string(),
  implementationStatus: z.string(),
  bashRequired: z.boolean(),
});

/**
 * Shell files the cutover keeps. `install.sh` is the curl installer users pipe
 * into bash before toolu or Bun exist (#457). The Jev shim is one `exec` line
 * until the launcher stops publishing it (#440).
 */
const SHELL_KEEP = new Set(["install.sh", "plugins/jev/scripts/jev.sh"]);

/** A row may still be a Bun bundle, or the generated native launcher. */
const HOST_MECHANISMS = new Set(["bun-bundle", "native"]);

function trackedShellFiles(): string[] {
  const result = spawnSync("git", ["ls-files", "-z", "*.sh", "*.bash", "*.bats"], {
    cwd: ROOT,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    throw new Error(`git ls-files failed: ${result.stderr.trim()}`);
  }
  return result.stdout.split("\0").filter((file) => file !== "" && !SHELL_KEEP.has(file));
}

function check(): void {
  const problems: string[] = [];
  for (const file of trackedShellFiles()) problems.push(`tracked shell file: ${file}`);
  if (existsSync(resolve(ROOT, ".shellcheckrc"))) problems.push(".shellcheckrc remains");
  if (existsSync(resolve(ROOT, "tooling/testdata/bats")))
    problems.push("tooling/testdata/bats remains");

  const packageRaw: unknown = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8"));
  const scripts = z
    .object({ scripts: z.record(z.string(), z.unknown()) })
    .parse(packageRaw).scripts;
  for (const name of ["lint:shell", "test:shell", "test:shell:serial"]) {
    if (name in scripts) problems.push(`shell-only package script remains: ${name}`);
  }

  const workflow = readFileSync(resolve(ROOT, ".github/workflows/tests.yml"), "utf8");
  for (const job of ["shellcheck", "bats"]) {
    if (new RegExp(`^  ${job}:`, "m").test(workflow)) {
      problems.push(`retired CI job remains: ${job}`);
    }
  }
  if (!/^  typescript:/m.test(workflow)) problems.push("replacement typescript CI job missing");

  const raw: unknown = JSON.parse(
    readFileSync(resolve(ROOT, "fixtures/gate-coverage/inventory.json"), "utf8"),
  );
  const inventory = z.array(Row).parse(raw);
  if (inventory.length === 0) problems.push("coverage inventory is empty");
  for (const row of inventory) {
    if (row.classification !== "port-native")
      problems.push(`${row.id}: classification ${row.classification}`);
    if (row.bashRequired) problems.push(`${row.id}: bashRequired=true`);
    if (!HOST_MECHANISMS.has(row.hostMechanism))
      problems.push(`${row.id}: hostMechanism ${row.hostMechanism}`);
    if (row.implementationStatus !== "done")
      problems.push(`${row.id}: implementationStatus ${row.implementationStatus}`);
  }

  const matrix = readFileSync(resolve(ROOT, "docs/gate-coverage-matrix.md"), "utf8");
  const matrixRows = matrix.split("\n").filter((line) => line.startsWith("| `"));
  if (matrixRows.length !== inventory.length)
    problems.push(`matrix has ${matrixRows.length} rows, inventory has ${inventory.length}`);
  for (const line of matrixRows) {
    const fields = line.match(
      /^\| `[^`]+` \| `[^`]+` \| [^|]+ \| [^|]+ \| ([^|]+) \| [^|]+ \| [^|]+ \| ([^|]+) \| ([^|]+) \|/,
    );
    if (!fields) {
      problems.push(`matrix row malformed: ${line}`);
      continue;
    }
    if (fields[1]?.trim() !== "port-native") problems.push(`matrix row is not native: ${line}`);
    if (!HOST_MECHANISMS.has(fields[2]?.trim() ?? ""))
      problems.push(`matrix row is not Bun or native: ${line}`);
    if (fields[3]?.trim() !== "no") problems.push(`matrix row requires Bash: ${line}`);
  }

  if (problems.length > 0) {
    for (const problem of problems) process.stderr.write(`final-removal: ${problem}\n`);
    process.exitCode = 1;
  } else {
    process.stdout.write(
      `final-removal: ok (${inventory.length} native rows, shell kept: ${[...SHELL_KEEP].join(", ")})\n`,
    );
  }
}

check();
