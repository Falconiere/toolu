/**
 * ast-grep's PostToolUse registry module (#268): measures how many bytes
 * Read, Grep, Glob and ast-grep run in Bash put into context, so the claim
 * that ast-grep returns far less than reading whole files is measured. It
 * records what each tool actually returned, plus a single-file Read's full
 * size, one line per call in a per-session ledger. Never blocks, never speaks.
 */
import { appendFileSync, mkdirSync } from "node:fs";
import { join } from "node:path";
import {
  defineRegistryModule,
  type RegistryContext,
  type RegistryHookEvent,
} from "@toolu/core/registry";
import {
  ledgerSessionId,
  readFullBytes,
  returnedBytes,
  savingsKind,
} from "./lib/savings-measure.ts";

/** `${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}/toolu/byte-savings`. */
function ledgerDir(env: RegistryContext["env"]): string {
  const root =
    env["TOOLU_CONFIG_DIR"] ||
    env["CODEX_HOME"] ||
    env["CLAUDE_CONFIG_DIR"] ||
    `${env["HOME"] ?? ""}/.claude`;
  return join(root, "toolu", "byte-savings");
}

function record(event: RegistryHookEvent, ctx: RegistryContext): void {
  const kind = savingsKind(event.toolName, event.toolInput["command"]);
  if (kind === undefined) return;
  const returned = returnedBytes(ctx.raw["tool_response"]);
  if (returned === undefined) return;
  const full =
    kind === "read" ? readFullBytes(event.toolInput["file_path"], ctx.cwd ?? process.cwd()) : 0;
  const dir = ledgerDir(ctx.env);
  try {
    mkdirSync(dir, { recursive: true });
    appendFileSync(
      join(dir, `${ledgerSessionId(ctx.raw["session_id"])}.jsonl`),
      `{"kind":"${kind}","returned":${returned},"full":${full}}\n`,
    );
  } catch {
    // The ledger is best-effort instrumentation; an unwritable root records nothing.
  }
}

export default defineRegistryModule({
  spec: "ast-grep@toolu",
  name: "byte-savings",
  event: "tool/post",
  run(event, ctx) {
    record(event, ctx);
    return Promise.resolve({ kind: "allow" });
  },
});
