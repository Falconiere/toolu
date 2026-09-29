/** Language-agnostic log summarizer for the debug skill's Observe step.
 *
 * Reads a log (stdin or --file) and emits a compact, capped summary: deduped
 * error/warning lines plus a tail of recent lines, always reporting TOTAL lines
 * so the caller knows what was elided. Its primary job is cap enforcement — a
 * huge log in, a small summary out, never a flood. Output stays at/under
 * DEBUG_MAX_LINES lines AND under DEBUG_MAX_BYTES bytes.
 *
 * Usage: bun debug-log.ts [--file <path>] [--json] */

import { envCap, jsonList, runHelper } from "./debug-io.ts";

const USAGE = `Usage: debug-log.ts [--file <path>] [--json]
  Reads a log from stdin or --file and prints a compact, capped summary:
  deduped ERROR/WARN-ish lines and a tail of the most recent lines.
  --json    Emit a JSON object {errors,tail,total_lines,truncated} instead of text.
Env caps (override): DEBUG_MAX_LINES=100 (max output lines) DEBUG_MAX_BYTES=65536 (hard output byte ceiling)
`;

const ERRORISH = /error|warn|panic|fail|exception|fatal|traceback/i;

/** Normalize a leading timestamp to a placeholder so timestamped repeats collapse on dedup. */
function dedupKey(line: string): string {
  return line
    .replace(/^\d{4}-\d{2}-\d{2}[T ][0-9:.]+([Zz]|[+-][0-9:]+)?/, "<TS>") // ISO 8601
    .replace(/^\[\d{2}:\d{2}:\d{2}([.,]\d+)?\]/, "<TS>"); // [12:00:00]
}

const jsonClean = (s: string) => s.replaceAll("\t", " ").replaceAll("\r", "");

export type LogCaps = { maxLines: number; maxBytes: number };

export function summarizeLog(lines: string[], json: boolean, caps: LogCaps): string {
  const { maxLines, maxBytes } = caps;
  const total = lines.length;
  const errs: string[] = [];
  const seen = new Set<string>();
  const kept = lines.map((raw) => raw.replace(/\r$/, ""));
  for (const line of kept) {
    if (!ERRORISH.test(line)) continue;
    const key = dedupKey(line);
    if (seen.has(key)) continue;
    seen.add(key);
    errs.push(line);
  }

  // Budget: errors get up to ~60% of the line cap, tail gets the rest (header lines count too).
  const emax = Math.max(Math.trunc(maxLines * 0.6), 1);
  const eshow = Math.min(errs.length, emax);
  const eover = errs.length - eshow;
  // Lines spent by the error section, its headers and the TOTAL line; -1 more for the TAIL header.
  const used = eshow + (errs.length > 0 ? 1 : 0) + (eover > 0 ? 1 : 0) + 1;
  const tmax = Math.max(maxLines - used - 1, 0);
  // Tail candidates: the last maxLines input lines.
  const tline = kept.slice(Math.max(total - maxLines, 0));
  const tn = tline.length;
  const tshow = Math.min(tn, tmax);
  const tail = tline.slice(tn - tshow);

  if (json) {
    const truncated = eover > 0 || tshow < tn || total > maxLines;
    return `{"errors":${jsonList(errs.slice(0, eshow), jsonClean)},"tail":${jsonList(tail, jsonClean)},"total_lines":${total},"truncated":${truncated}}\n`;
  }

  // awk prints its uninitialized counters as "" — kept for byte parity with debug-log.sh.
  const eshowLabel = errs.length === 0 ? "" : String(eshow);
  const totalLabel = total === 0 ? "" : String(total);
  const out = [`ERRORS/WARNINGS (${eshowLabel}${eover > 0 ? "+" : ""}):`];
  for (const line of errs.slice(0, eshow)) out.push(`  ${line}`);
  if (eover > 0) out.push(`  ... (+${eover} more)`);
  out.push(`TAIL (last ${tshow} of ${totalLabel} lines):`);
  for (const line of tail) out.push(`  ${line}`);
  out.push(`TOTAL lines: ${totalLabel}`);

  // Hard byte ceiling (one char per byte): drop trailing lines rather than exceed maxBytes.
  let bytes = 0;
  let keep = out.length;
  for (let i = 0; i < out.length; i++) {
    bytes += (out[i]?.length ?? 0) + 1;
    if (bytes > maxBytes) {
      keep = i;
      break;
    }
  }
  const shown = out.slice(0, keep);
  if (keep < out.length) shown.push(`... (output truncated to stay under ${maxBytes} bytes)`);
  return shown.map((line) => `${line}\n`).join("");
}

if (import.meta.main) {
  const caps = {
    maxLines: envCap("DEBUG_MAX_LINES", 100),
    maxBytes: envCap("DEBUG_MAX_BYTES", 65536),
  };
  process.exit(
    runHelper(
      {
        name: "debug-log.ts",
        usage: USAGE,
        inputNoun: "a log",
        summarize: (lines, json) => summarizeLog(lines, json, caps),
      },
      process.argv.slice(2),
    ),
  );
}
