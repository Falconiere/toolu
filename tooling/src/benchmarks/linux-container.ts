/**
 * Reproduce the final hook comparison in a disposable Linux Bun container.
 * The checkout includes committed files and tracked working-tree changes only.
 * Add new source files to Git before running this diagnostic; untracked files
 * are absent from both the clone and `git diff HEAD`.
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../../..");
const IMAGE = "toolu-latency:1.4.2";

function run(argv: string[], cwd: string, input?: string): string {
  const result = spawnSync(argv[0] ?? "", argv.slice(1), {
    cwd,
    ...(input === undefined ? {} : { input }),
    encoding: "utf8",
    timeout: 600_000,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (result.error) throw new Error(`${argv.join(" ")}: ${result.error.message}`);
  if (result.status !== 0)
    throw new Error(`${argv.join(" ")} exited ${String(result.status)}: ${result.stderr}`);
  return result.stdout;
}

function main(argv: string[]): void {
  const at = argv.indexOf("--runs");
  const runs = at === -1 ? "15" : argv[at + 1];
  if (!runs || !/^[1-9][0-9]*$/.test(runs)) throw new Error("--runs needs a positive integer");
  const parent = resolve(ROOT, "node_modules/.cache");
  const temp = mkdtempSync(join(parent, "toolu-linux-bench-"));
  const clone = join(temp, "checkout");
  try {
    run(["git", "clone", "--quiet", "--no-hardlinks", ROOT, clone], ROOT);
    // `git diff HEAD` includes staged and unstaged edits, but never untracked files.
    const patch = run(["git", "diff", "--binary", "HEAD"], ROOT);
    if (patch !== "") run(["git", "apply", "--binary", "-"], clone, patch);
    run(
      [
        "docker",
        "build",
        "--quiet",
        "-f",
        join(ROOT, "tooling/latency.Dockerfile"),
        "-t",
        IMAGE,
        join(ROOT, "tooling"),
      ],
      ROOT,
    );
    const base = [
      "docker",
      "run",
      "--rm",
      "-v",
      `${clone}:/repo`,
      "-v",
      `${join(ROOT, "node_modules")}:/repo/node_modules:ro`,
      "-w",
      "/repo",
      "-e",
      "GIT_CONFIG_COUNT=1",
      "-e",
      "GIT_CONFIG_KEY_0=safe.directory",
      "-e",
      "GIT_CONFIG_VALUE_0=/repo",
      IMAGE,
      "run",
    ];
    for (const file of ["final-hook-latency.ts", "post-tools-latency.ts"]) {
      const output = run([...base, `tooling/src/benchmarks/${file}`, "--runs", runs], ROOT);
      process.stdout.write(output);
    }
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

main(process.argv.slice(2));
