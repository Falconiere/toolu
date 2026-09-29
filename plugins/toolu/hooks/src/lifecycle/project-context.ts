/**
 * The two project inputs of toolu's UserPromptSubmit hook (#263): the failing
 * quality-gate reminder and the opt-in `<project>/<host dir>/context.sh`.
 */
import { existsSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { jqAlt, member, parseStdin, stripTrailingNewlines } from "./bash-compat.ts";

type Env = Record<string, string | undefined>;

function isFile(path: string): boolean {
  try {
    return statSync(path).isFile();
  } catch {
    return false;
  }
}

/** `Quality gate failing: <reason>. …` when the gate file says `failing`, else undefined. */
export function failingGateHint(stateRoot: string): string | undefined {
  const file = join(stateRoot, "quality-gate-status.json");
  if (!isFile(file)) return undefined;
  let doc: unknown;
  try {
    doc = parseStdin(readFileSync(file, "utf8"));
  } catch {
    return undefined;
  }
  if (member(doc, "status") !== "failing") return undefined;
  const reason = jqAlt(member(doc, "reason"), "Unknown quality failure");
  return `Quality gate failing: ${reason}. Prefer fixing before unrelated work.`;
}

/** `PROMPT="$prompt" bash context.sh 2>/dev/null || true`, trailing newlines dropped. */
export function projectContext(script: string, prompt: string, cwd: string, env: Env): string {
  if (!existsSync(script) || !isFile(script)) return "";
  const bash = Bun.which("bash", { PATH: env.PATH ?? "" });
  if (bash === null) return "";
  const res = Bun.spawnSync([bash, script], {
    cwd,
    env: { ...env, PROMPT: prompt },
    stdin: "ignore",
    stdout: "pipe",
    stderr: "ignore",
  });
  return stripTrailingNewlines(res.stdout.toString());
}
