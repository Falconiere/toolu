/**
 * patterns — contextual code patterns via ast-grep: "bare fetch, EXCEPT inside
 * utilities/http.ts" is not a grep. One rule syntax across TS, TSX and Rust.
 *
 * ONE ast-grep process per run (its startup dwarfs the scan). ast-grep exits
 * 0 = no match, 1 = a rule matched, anything else = ast-grep failed (8 =
 * unparseable rule, 6 = missing config); that last case fails the gate closed
 * rather than letting one malformed rule silence every pattern check.
 */
import { spawnSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import type { GuardrailsConfig } from "../config.ts";
import type { CheckContext } from "../context.ts";
import { ASSETS } from "../paths.ts";
import { fatal } from "../report.ts";
import { jqText } from "../shape.ts";
import { isDir } from "../walk.ts";

/** Local devDependency first (starts in ms), then PATH, then bunx. */
function astGrep(root: string): string[] | null {
  const local = join(root, "node_modules/.bin/ast-grep");
  if (existsSync(local)) return [local];
  if (Bun.which("ast-grep") !== null) return ["ast-grep"];
  if (Bun.which("bunx") !== null) return ["bunx", "--bun", "@ast-grep/cli"];
  return null;
}

type Match = { ruleId: string; file: string; message: string };

function parseMatches(output: string): Match[] {
  const parsed: unknown = JSON.parse(output);
  if (!Array.isArray(parsed)) return [];
  return parsed.map((entry: unknown) => {
    const field = (key: string): string =>
      jqText(typeof entry === "object" && entry !== null ? Reflect.get(entry, key) : null);
    return { ruleId: field("ruleId"), file: field("file"), message: field("message") };
  });
}

export function patterns(
  ctx: CheckContext<GuardrailsConfig>,
  mode: "repo" | "batch",
  paths: readonly string[],
): void {
  const rules = join(ASSETS, "patterns");
  if (!existsSync(join(rules, "sgconfig.yml"))) return;
  const bin = astGrep(ctx.root);
  if (bin === null) {
    fatal(
      "ast-grep not found — the pattern checks cannot run (add @ast-grep/cli as a devDependency, or: cargo install ast-grep --locked)",
    );
  }
  let targets: readonly string[];
  if (mode === "batch") {
    if (paths.length === 0) return;
    targets = paths;
  } else {
    if (!isDir(ctx.root, ctx.config.srcRoot)) return;
    targets = [ctx.config.srcRoot];
  }
  const [cmd = "", ...args] = bin;
  const res = spawnSync(
    cmd,
    [...args, "scan", "-c", join(rules, "sgconfig.yml"), "--json=compact", ...targets],
    { cwd: ctx.root, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 },
  );
  if (res.error) throw res.error;
  const status = res.status ?? 1;
  if (status !== 0 && status !== 1) {
    fatal(
      `ast-grep exited ${String(status)} scanning ${targets.join(" ")} — a rule in ${rules} is malformed or unreadable; the pattern checks did NOT run`,
    );
  }
  if (res.stdout === "" || res.stdout.trim() === "[]") return;
  for (const match of parseMatches(res.stdout)) {
    ctx.report.violation("patterns", match.file, match.ruleId, match.message);
  }
}
