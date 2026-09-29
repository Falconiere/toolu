/** Language-agnostic stack/backtrace summarizer for the debug skill's Observe step.
 *
 * Reads a stack trace / panic backtrace (stdin or --file) and surfaces APP frames
 * first, collapsing framework/runtime/stdlib frames into a count. Never a
 * per-language grammar — it keys on line SHAPE (a symbol and/or a
 * file:line[:col]) and flags noise by path/symbol markers (/rustc/,
 * node:internal, core::, std::, __rustc, <anonymous>, …). JS (symbol+loc on one
 * line) and Rust (symbol then `at <path>` on the next line) both work.
 * Unrecognized input → capped raw passthrough.
 *
 * Usage: bun debug-stack.ts [--file <path>] [--json] */

import { envCap, jsonList, runHelper, utf8Bytes } from "./debug-io.ts";

const USAGE = `Usage: debug-stack.ts [--file <path>] [--json]
  Reads a stack trace / backtrace from stdin or --file and prints APP frames first, runtime noise collapsed.
  --json    Emit {"app_frames":[...],"noise_frames":N,"recognized":bool} instead of text.
Env caps (override): DEBUG_MAX_FRAMES=20 DEBUG_MAX_LINES=100
`;

/** A frame is NOISE when its symbol or location points into a runtime/stdlib. */
const NOISE =
  /\/rustc\/|node:internal|node:|core::|std::|__rustc|rust_begin_unwind|panic_fmt|panic_bounds_check|FnOnce::call_once|<anonymous>/;
const AT = /^[ \t]*at[ \t]+/;
const RUST_INDEX = /^[ \t]*[0-9]+:[ \t]+/;
/** `(loc)` at the end of a JS frame; `s` so `.` also spans a CR, as in awk. */
const PAREN_LOC = /\(.*\)[ \t]*$/s;

const trim = (s: string) => s.replace(/^[ \t]+/, "").replace(/[ \t]+$/, "");

type StackCaps = { maxFrames: number; maxRaw: number };

type Frames = { app: string[]; noise: number };

function parseFrames(lines: string[]): Frames {
  const app: string[] = [];
  let noise = 0;
  const emit = (sym: string, loc: string) => {
    if (NOISE.test(`${sym} ${loc}`)) {
      noise++;
      return;
    }
    if (sym !== "" && loc !== "") app.push(`${sym} @ ${loc}`);
    else app.push(loc !== "" ? loc : sym);
  };
  let pending: string | null = null;
  for (const line of lines) {
    const at = AT.exec(line);
    // Rust continuation FIRST: `             at <location>` pairs with the pending `N: symbol`.
    if (pending !== null && at !== null) {
      emit(pending, trim(line.slice(at[0].length)));
      pending = null;
      continue;
    }
    // A pending Rust symbol with no following `at` line: still a frame, no location.
    if (pending !== null) {
      emit(pending, "");
      pending = null;
    }
    // JS shape: `at <symbol> (<location>)` or `at <location>`.
    if (at !== null) {
      const rest = line.slice(at[0].length);
      let sym = "";
      let loc: string;
      const paren = PAREN_LOC.exec(rest);
      if (paren !== null) {
        sym = trim(rest.slice(0, paren.index));
        loc = rest.slice(paren.index + 1, paren.index + paren[0].length - 1).replace(/[ \t]+$/, "");
      } else {
        loc = trim(rest);
      }
      if (sym !== "" || loc !== "") {
        emit(sym, loc);
        continue;
      }
    }
    // Rust shape: `   N: <symbol>`, then the NEXT line `             at <location>`.
    const index = RUST_INDEX.exec(line);
    if (index !== null) pending = trim(line.slice(index[0].length));
  }
  if (pending !== null) emit(pending, "");
  return { app, noise };
}

const jsonClean = (s: string) => s.replaceAll("\t", " ");

function summarizeStack(lines: string[], json: boolean, caps: StackCaps): string {
  const { app, noise } = parseFrames(lines);
  // Noise frames count too: a trace of only runtime frames is still a trace (as in the bash version).
  const recognized = app.length + noise > 0;
  if (json) {
    return `{"app_frames":${jsonList(app, jsonClean)},"noise_frames":${noise},"recognized":${recognized}}\n`;
  }
  const out: string[] = [];
  if (!recognized) {
    out.push(`debug-stack: no recognizable stack frames ${utf8Bytes("—")} raw input (capped):`);
    out.push(...lines.slice(0, Math.max(Math.min(lines.length, caps.maxRaw), 0)));
    if (lines.length > caps.maxRaw)
      out.push(`... (+${Math.trunc(lines.length - caps.maxRaw)} more lines)`);
    return out.map((line) => `${line}\n`).join("");
  }
  const over = app.length > caps.maxFrames;
  // With only noise frames, awk printed its uninitialized frame counter as "" (`APP FRAMES ():`);
  // kept for byte parity with the bash version.
  out.push(`APP FRAMES (${app.length === 0 ? "" : app.length}${over ? "+" : ""}):`);
  for (const frame of app.slice(0, Math.max(Math.min(app.length, caps.maxFrames), 0))) {
    out.push(`  - ${frame}`);
  }
  if (over) out.push(`  ... (+${Math.trunc(app.length - caps.maxFrames)} more)`);
  if (noise > 0) out.push(`(+${noise} framework/runtime frames collapsed)`);
  return out.map((line) => `${line}\n`).join("");
}

if (import.meta.main) {
  const caps = {
    maxFrames: envCap("DEBUG_MAX_FRAMES", 20),
    maxRaw: envCap("DEBUG_MAX_LINES", 100),
  };
  process.exit(
    runHelper(
      {
        name: "debug-stack.ts",
        usage: USAGE,
        inputNoun: "a trace",
        summarize: (lines, json) => summarizeStack(lines, json, caps),
      },
      process.argv.slice(2),
    ),
  );
}
