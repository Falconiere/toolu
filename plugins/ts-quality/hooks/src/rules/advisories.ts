/**
 * The non-blocking advisories (#265): code duplication through the project's
 * jscpd (90), missing or verbose JSDoc on exported API (92), and `await`
 * with no handler in the file (94). They never touch the gate.
 */
import { spawnSync } from "node:child_process";
import { statSync } from "node:fs";
import { basename, join } from "node:path";
import { toolAvailable } from "@toolu/core/detect";
import { ere, head, isTestPath, spawnEnv, type TsFile } from "./ts-file.ts";

/** `bun run typecheck` and friends: what the duplication advisory tells the agent to run. */
export function typecheckCommand(pm: string): string {
  if (pm === "bun") return "bun run typecheck";
  if (pm === "pnpm") return "pnpm -w typecheck";
  if (pm === "yarn") return "yarn typecheck";
  if (pm === "npm") return "npm run typecheck";
  return `${pm} run typecheck`;
}

const RUNNERS: Readonly<Record<string, readonly [string, readonly string[]]>> = {
  bun: ["bunx", ["bunx", "jscpd"]],
  pnpm: ["pnpm", ["pnpm", "dlx", "jscpd"]],
  yarn: ["yarn", ["yarn", "dlx", "jscpd"]],
  npm: ["npx", ["npx", "jscpd"]],
};

function kindOf(path: string): "file" | "dir" | "none" {
  try {
    const stat = statSync(path);
    return stat.isFile() ? "file" : stat.isDirectory() ? "dir" : "none";
  } catch {
    return "none";
  }
}

/**
 * A basic regular expression (grep's default) as a JavaScript one: `+?(){}|`
 * are literals there, and `^`/`$` anchor only at the pattern's start/end.
 */
function bre(text: string): RegExp {
  const chars = [...text];
  const escaped = chars.map((char, at) => {
    if ("+?(){}|".includes(char)) return `\\${char}`;
    if (char === "^" && at > 0) return String.raw`\^`;
    if (char === "$" && at < chars.length - 1) return String.raw`\$`;
    return char;
  });
  return new RegExp(escaped.join(""), "s");
}

/** `timeout 10 <runner> <pkg> --config .jscpd.json 2>&1`, both streams. */
function jscpdOutput(f: TsFile, runner: readonly string[], pkg: string, config: string): string {
  const res = spawnSync("timeout", ["10", ...runner, pkg, "--config", config], {
    cwd: f.ctx.cwd,
    env: spawnEnv(f),
    encoding: "utf8",
  });
  const out = res.stdout ?? "";
  return `${out}${out === "" || out.endsWith("\n") ? "" : "\n"}${res.stderr ?? ""}`;
}

/** 90-duplication: only for a clean non-test file under `apps/*` or `packages/*`. */
export function duplication(f: TsFile): string {
  const path = f.file.path;
  if (/\.(test|spec)\./s.test(path)) return "";
  const root = f.ctx.projectRoot;
  const relative = path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
  if (!relative.startsWith("apps/") && !relative.startsWith("packages/")) return "";
  const pkg = `${root}/${relative.split("/").slice(0, 2).join("/")}`;
  const runner = RUNNERS[f.pm];
  const config = join(root, ".jscpd.json");
  if (runner === undefined || !toolAvailable(runner[0], f.ctx.env)) return "";
  if (kindOf(pkg) !== "dir" || kindOf(config) !== "file") return "";
  const lines = jscpdOutput(f, runner[1], pkg, config).split("\n");
  const cloned = lines.some((line) => /found.*clone|duplicat/is.test(line));
  const named = lines.some((line) => bre(basename(path)).test(line));
  if (!cloned || !named) return "";
  return `Code duplication detected involving ${path} — deduplicate or run '${typecheckCommand(f.pm)}' before commit`;
}

const EXPORTS = [
  /^export (async )?function /s,
  /^export (abstract )?class /s,
  /^export default /s,
  /^export (const|interface|type|enum) [A-Z]/s,
  /^export const [a-z_][A-Za-z0-9_]* = (async )?(\(|function)/s,
];

/** Exported API whose previous code line neither closes nor opens a JSDoc. */
function undocumented(lines: readonly string[]): string[] {
  const rows: string[] = [];
  const blank = ere("^[[:space:]]*$");
  const lineComment = ere("^[[:space:]]*//");
  let prev = "";
  lines.forEach((line, i) => {
    if (blank.test(line) || lineComment.test(line)) return;
    const documented =
      ere(String.raw`\*/[[:space:]]*$`).test(prev) ||
      ere(String.raw`^[[:space:]]*/\*\*`).test(prev);
    if (EXPORTS.some((pattern) => pattern.test(line)) && !documented)
      rows.push(`${String(i + 1)}: ${line}`);
    prev = line;
  });
  return rows;
}

/** JSDoc blocks longer than 12 lines; a `/**` inside a block does not restart it. */
function verbose(lines: readonly string[]): string[] {
  const rows: string[] = [];
  let block: { start: number; count: number } | undefined;
  lines.forEach((line, i) => {
    if (block === undefined && line.includes("/**")) block = { start: i + 1, count: 0 };
    if (block === undefined) return;
    block.count += 1;
    if (!line.includes("*/")) return;
    if (block.count > 12)
      rows.push(
        `${String(block.start)}: JSDoc block is ${String(block.count)} lines — trim to the essentials`,
      );
    block = undefined;
  });
  return rows;
}

/** 92-docs: skips tests, declarations and `index.ts(x)` barrels. */
export function docs(f: TsFile): string {
  const path = f.file.path;
  const name = basename(path);
  if (isTestPath(path) || /\.d\.ts$/s.test(path) || name === "index.ts" || name === "index.tsx")
    return "";
  const missing = head(undocumented(f.lines), 3);
  const long = head(verbose(f.lines), 2);
  const parts: string[] = [];
  if (missing !== "")
    parts.push(`Exported API missing a JSDoc in ${path} — add a concise /** */ doc:\n${missing}`);
  if (long !== "")
    parts.push(`Verbose JSDoc in ${path} — docs must be present but concise:\n${long}`);
  return parts.join("\n");
}

/** 94-handler: `await` in code lines, and no `try` or `.catch(` in any. */
export function unhandledAwait(f: TsFile): string {
  const comment = ere(String.raw`^[[:space:]]*(//|/\*|\*)`);
  const code = f.lines.filter((line) => !comment.test(line));
  const awaits = code.some((line) => ere(String.raw`\bawait[[:space:]]`).test(line));
  const handled = code.some((line) => /\btry\b|\.catch\(/s.test(line));
  if (!awaits || handled) return "";
  return `Async code in ${f.file.path} uses await with no try/catch or .catch in the file — ensure rejections are handled here or by every caller.`;
}
