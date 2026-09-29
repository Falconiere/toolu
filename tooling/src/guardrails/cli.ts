/**
 * Argument parsing, identical to the bash runner:
 *   (none)          repo mode — every check, whole tree. The gate.
 *   --file <path>…  file-addressable checks for those paths (lefthook's {staged_files})
 *   --hook          PostToolUse — hook JSON on stdin, then the edited file
 *   --stop          Stop hook — repo mode behind two early-outs
 *   --only a,b      run only the named checks
 *   --list          print every check id, one per line
 */
import { fatal } from "./report.ts";

export type RunMode = "repo" | "file" | "hook" | "stop";
export type Options = { mode: RunMode; paths: string[]; only: string; list: boolean };

export function parseArgs(argv: readonly string[]): Options {
  const opts: Options = { mode: "repo", paths: [], only: "", list: false };
  let i = 0;
  while (i < argv.length) {
    const arg = argv[i] ?? "";
    if (arg === "--list") return { ...opts, list: true };
    if (arg === "--file") {
      opts.mode = "file";
      i += 1;
      while (i < argv.length && !(argv[i] ?? "").startsWith("--")) {
        opts.paths.push(argv[i] ?? "");
        i += 1;
      }
      if (opts.paths.length === 0) fatal("--file needs at least one path");
    } else if (arg === "--hook" || arg === "--stop") {
      opts.mode = arg === "--hook" ? "hook" : "stop";
      i += 1;
    } else if (arg === "--only") {
      opts.only = argv[i + 1] ?? "";
      if (opts.only === "") fatal("--only needs a comma-separated check list");
      i += 2;
    } else {
      fatal(`unknown flag: ${arg} (try --list)`);
    }
  }
  return opts;
}
