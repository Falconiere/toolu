/** Language-agnostic test-failure summarizer for the debug skill's Observe step.
 *
 * Reads a failing test transcript (stdin or --file), emits a compact, capped
 * summary: failed test names, error/assertion lines, and code file:line
 * locations. Never a per-language grammar — it keys on line shape only (works on
 * bun/jest, cargo, and similar). Unrecognized input → capped raw passthrough.
 *
 * Usage: bun debug-testfail.ts [--file <path>] [--json] */

import { envCap, jsonList, runHelper, utf8Bytes } from "./debug-io.ts";

const USAGE = `Usage: debug-testfail.ts [--file <path>] [--json]
  Reads a test-failure transcript from stdin or --file and prints a compact summary.
  --json    Emit a JSON object {failures,errors,locations} instead of text.
Env caps (override): DEBUG_MAX_FAILURES=25 DEBUG_MAX_ERRORS=25 DEBUG_MAX_LOCATIONS=25 DEBUG_MAX_LINES=100
`;

const BUN_FAIL = /\(fail\)[ \t]+/; // bun/jest:  (fail) NAME [1ms]
const BUN_DURATION = /[ \t]*\[[^\]]*\][ \t]*$/;
const CARGO_FAIL = /(^|[ \t])test[ \t]+[^ \t]+[ \t]+\.\.\.[ \t]+FAILED/; // cargo: test NAME ... FAILED
const GENERIC_FAIL = new RegExp(
  `^[ \\t]*(FAIL|${utf8Bytes("✕")}|${utf8Bytes("✗")}|${utf8Bytes("×")})[ \\t]+`,
);
/** `s` so `.` also spans a CR, as in awk. */
const ERROR_LINE =
  /([Ee]rror:|panicked at|[Aa]ssertion|AssertionError|Expected:|Received:|^[ \t]*left:|^[ \t]*right:|thread .* panicked)/s;
/** Code file:line[:col] locations: a path with an extension, then :digits. */
const LOCATION = /[A-Za-z0-9_./\\-]+\.[A-Za-z]+:[0-9]+(:[0-9]+)?/g;

const trim = (s: string) => s.replace(/^[ \t]+/, "").replace(/[ \t]+$/, "");

type TestfailCaps = {
  maxFailures: number;
  maxErrors: number;
  maxLocations: number;
  maxRaw: number;
};

type Bucket = "f" | "e" | "l";

type Summary = {
  failures: string[];
  errors: string[];
  locations: string[];
  over: Record<Bucket, number>;
};

function collect(lines: string[], caps: TestfailCaps): Summary {
  const s: Summary = { failures: [], errors: [], locations: [], over: { f: 0, e: 0, l: 0 } };
  const seen = {
    failures: new Set<string>(),
    errors: new Set<string>(),
    locations: new Set<string>(),
  };
  const add = (list: "failures" | "errors" | "locations", max: number, raw: string) => {
    const val = trim(raw);
    if (val === "" || seen[list].has(val)) return;
    if (s[list].length >= max) {
      // The overflow bucket is picked by comparing cap VALUES, as the bash version did:
      // with equal caps (the defaults) every overflow lands in the failures bucket.
      const bucket: Bucket = max === caps.maxFailures ? "f" : max === caps.maxErrors ? "e" : "l";
      s.over[bucket]++;
      return;
    }
    seen[list].add(val);
    s[list].push(val);
  };
  for (const line of lines) {
    const bun = BUN_FAIL.exec(line);
    const generic = GENERIC_FAIL.exec(line);
    if (bun !== null) {
      add(
        "failures",
        caps.maxFailures,
        line.slice(bun.index + bun[0].length).replace(BUN_DURATION, ""),
      );
    } else if (CARGO_FAIL.test(line)) {
      const name = line.replace(/^[ \t]*test[ \t]+/, "").replace(/[ \t]+\.\.\..*/s, "");
      add("failures", caps.maxFailures, name);
    } else if (generic !== null) {
      add("failures", caps.maxFailures, line.slice(generic[0].length));
    }
    if (ERROR_LINE.test(line)) add("errors", caps.maxErrors, line);
    for (const m of line.matchAll(LOCATION)) add("locations", caps.maxLocations, m[0]);
  }
  return s;
}

const jsonClean = (s: string) => s.replaceAll("\t", " ");

/** A section header count; awk printed its uninitialized counter as "". */
const count = (n: number) => (n === 0 ? "" : String(n));

function summarizeTestfail(lines: string[], json: boolean, caps: TestfailCaps): string {
  const { failures, errors, locations, over } = collect(lines, caps);
  const recognized = failures.length + errors.length + locations.length > 0;
  if (json) {
    return `{"failures":${jsonList(failures, jsonClean)},"errors":${jsonList(errors, jsonClean)},"locations":${jsonList(locations, jsonClean)},"recognized":${recognized}}\n`;
  }
  const out: string[] = [];
  if (!recognized) {
    out.push(`debug-testfail: no recognizable test failures ${utf8Bytes("—")} raw input (capped):`);
    out.push(...lines.slice(0, Math.max(Math.min(lines.length, caps.maxRaw), 0)));
    if (lines.length > caps.maxRaw)
      out.push(`... (+${Math.trunc(lines.length - caps.maxRaw)} more lines)`);
    return out.map((line) => `${line}\n`).join("");
  }
  const section = (title: string, items: string[], bucket: Bucket, prefix: string) => {
    out.push(`${title} (${count(items.length)}${over[bucket] > 0 ? "+" : ""}):`);
    for (const item of items) out.push(`${prefix}${item}`);
    if (over[bucket] > 0) out.push(`  ... (+${over[bucket]} more)`);
  };
  section("FAILED TESTS", failures, "f", "  - ");
  if (errors.length > 0) section("\nERRORS", errors, "e", "  ");
  if (locations.length > 0) section("\nLOCATIONS", locations, "l", "  ");
  return out.map((line) => `${line}\n`).join("");
}

if (import.meta.main) {
  const caps = {
    maxFailures: envCap("DEBUG_MAX_FAILURES", 25),
    maxErrors: envCap("DEBUG_MAX_ERRORS", 25),
    maxLocations: envCap("DEBUG_MAX_LOCATIONS", 25),
    maxRaw: envCap("DEBUG_MAX_LINES", 100),
  };
  process.exit(
    runHelper(
      {
        name: "debug-testfail.ts",
        usage: USAGE,
        inputNoun: "a transcript",
        summarize: (lines, json) => summarizeTestfail(lines, json, caps),
      },
      process.argv.slice(2),
    ),
  );
}
