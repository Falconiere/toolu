/**
 * ast-grep's PostToolUse registry module (#268): measures how many bytes
 * Read, Grep, Glob and ast-grep run in Bash put into context, so the claim
 * that ast-grep returns far less than reading whole files is measured. It
 * records what each tool actually returned, plus a single-file Read's full
 * size, one line per call in a per-session ledger. Never blocks. Silent on
 * every host but OpenCode, where an ast-grep run's result carries the
 * session's report (#347): its host contract delivers savings to the model.
 */
import { appendFileSync, mkdirSync, readFileSync } from "node:fs";
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
import { parseLedger, report } from "./lib/savings-report.ts";

/** `${TOOLU_CONFIG_DIR:-${CODEX_HOME:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}}/toolu/byte-savings`. */
function ledgerDir(env: RegistryContext["env"]): string {
  const root =
    env["TOOLU_CONFIG_DIR"] ||
    env["CODEX_HOME"] ||
    env["CLAUDE_CONFIG_DIR"] ||
    `${env["HOME"] ?? ""}/.claude`;
  return join(root, "toolu", "byte-savings");
}

/** The ledger the call was appended to, with its kind; undefined when nothing was recorded. */
function record(
  event: RegistryHookEvent,
  ctx: RegistryContext,
): { ledger: string; kind: string } | undefined {
  const kind = savingsKind(event.toolName, event.toolInput["command"]);
  if (kind === undefined) return undefined;
  const returned = returnedBytes(ctx.raw["tool_response"]);
  if (returned === undefined) return undefined;
  const full =
    kind === "read" ? readFullBytes(event.toolInput["file_path"], ctx.cwd ?? process.cwd()) : 0;
  const dir = ledgerDir(ctx.env);
  const ledger = join(dir, `${ledgerSessionId(ctx.raw["session_id"])}.jsonl`);
  try {
    mkdirSync(dir, { recursive: true });
    appendFileSync(ledger, `{"kind":"${kind}","returned":${returned},"full":${full}}\n`);
  } catch {
    // The ledger is best-effort instrumentation; an unwritable root records nothing.
    return undefined;
  }
  return { ledger, kind };
}

/** The session's report, or why it is unavailable; the record itself is already written. */
function sessionReport(ledger: string): string {
  let text: string;
  try {
    text = readFileSync(ledger, "utf8");
  } catch (error) {
    return `report unavailable: ${error instanceof Error ? error.message : String(error)}`;
  }
  const records = parseLedger(text);
  return Array.isArray(records)
    ? report(records)
    : `report unavailable: ${ledger}:${records.line}: invalid ledger line`;
}

export default defineRegistryModule({
  spec: "ast-grep@toolu",
  name: "byte-savings",
  event: "tool/post",
  run(event, ctx) {
    const recorded = record(event, ctx);
    if (ctx.host !== "opencode" || recorded?.kind !== "ast-grep") {
      return Promise.resolve({ kind: "allow" });
    }
    const message = `ast-grep byte savings this session:\n${sessionReport(recorded.ledger)}`;
    return Promise.resolve({ kind: "advisory", message });
  },
});
