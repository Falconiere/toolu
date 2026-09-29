/**
 * AC-1 (#256): `parseSteps`, `docField`, `parseAcs` and `checkAcRefs` against
 * the bash functions in `plan-ledger-parse.sh`. Each case writes one real
 * document into a sandbox and runs both implementations on it. They must
 * produce the same stdout, the same exit code and the same tagged stderr.
 * Inputs are the documents the bats suites build, plus a malformed catalogue.
 */
import { describe, expect, test } from "bun:test";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { MODEL_ALIASES } from "../../config/config-read.ts";
import { toJqJson } from "../../state/state-io.ts";
import { checkAcRefs, docField, parseAcs, parseSteps } from "../ledger-parse.ts";
import { LIB, bashFn, bashOutcome, type Outcome } from "./ledger-parity-helpers.ts";

const PARSE = "plan-ledger-parse.sh";
const fence = (body: string, heading = "## Steps (machine-readable)"): string =>
  `# Plan\n\n${heading}\n\n\`\`\`json\n${body}\n\`\`\`\n`;
const two =
  '[\n  { "id": "s1", "title": "First step", "check": "true" },\n  { "id": "s2", "title": "Second step", "check": "bats foo.bats" }\n]';

/** name → plan document body; `null` means the file is never written. */
const PLANS: Record<string, string | null> = {
  valid: `# Some Plan\n\n## Context\n\nProse:\n\n\`\`\`json\n{ "decoy": true }\n\`\`\`\n\n${fence(two)}\n## Verification\n\nMore prose.\n`,
  "missing heading": "# Some Plan\n\n## Context\n\nNo machine-readable steps here.\n",
  "empty array": fence("[]"),
  "malformed json": fence('[{"id":'),
  "missing check": fence('[\n  { "id": "s1", "title": "First step" }\n]'),
  "empty id": fence('[\n  { "id": "", "title": "First step", "check": "true" }\n]'),
  absent: null,
  normalized: fence(
    '[\n  { "id": "s1", "title": "Enriched step", "check": "true",\n    "ac_refs": ["AC-1", "AC-2"], "depends_on": ["s0"], "input": "fixture docs" },\n  { "id": "s2", "title": "Legacy step", "check": "false" }\n]',
  ),
  "every alias": fence(
    `[${MODEL_ALIASES.map((m, i) => `{ "id": "s${i}", "title": "t", "check": "true", "model": "${m}" }`).join(",")}]`,
  ),
  "unroutable model": fence('[{ "id": "s1", "title": "Bad", "check": "true", "model": "gpt-9" }]'),
  "numeric model": fence('[{ "id": "s1", "title": "Bad", "check": "true", "model": 5 }]'),
  "false model": fence('[{ "id": "s1", "title": "Bad", "check": "true", "model": false }]'),
  "null model": fence('[{ "id": "s1", "title": "U", "check": "true", "model": null }]'),
  "two bad models": fence(
    '[{ "id": "a", "title": "t", "check": "c", "model": "x" }, { "id": "b", "title": "t", "check": "c", "model": {"k": 1} }]',
  ),
  "paths string": fence('[{ "id": "s1", "title": "t", "check": "true", "paths": "src" }]'),
  "paths empty member": fence(
    '[{ "id": "s1", "title": "t", "check": "true", "paths": ["a", ""] }]',
  ),
  "paths ok": fence('[{ "id": "s1", "title": "t", "check": "true", "paths": ["-weird", "src"] }]'),
  "falsy authored fields": fence(
    '[{ "id": "s1", "title": "t", "check": "true", "ac_refs": false, "depends_on": null, "input": false }]',
  ),
  "non-object step": fence('["s1"]'),
  "null step": fence("[null]"),
  "object block": fence('{ "id": "s1", "title": "t", "check": "true" }'),
  "whitespace-only block": fence("   "),
  "blank-line block": fence("\n\n"),
  "crlf heading and fences": `# Plan\r\n\r\n## Steps (machine-readable)\r\n\r\n\`\`\`json\r\n[{ "id": "s1", "title": "t", "check": "true" }]\r\n\`\`\`\r\n`,
  "heading with trailing spaces": fence(
    '[{ "id": "s1", "title": "t", "check": "true" }]',
    "## Steps (machine-readable)   ",
  ),
  "unclosed fence":
    '## Steps (machine-readable)\n\n```json\n[{ "id": "s1", "title": "t", "check": "true" }]\n',
  "second json block ignored": `${fence('[{ "id": "s1", "title": "t", "check": "true" }]')}\n\`\`\`json\n[]\n\`\`\`\n`,
  "unicode and DEL": fence('[{ "id": "é", "title": "a\\u007fb ✓", "check": "printf x" }]'),
};

describe("parseSteps vs pl_parse_steps", () => {
  for (const [name, body] of Object.entries(PLANS)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      if (body !== null) sb.write("plan.md", body);
      const bash = await bashFn(PARSE, "pl_parse_steps", ["plan.md"], { cwd: sb.project });
      const ts = parseSteps("plan.md", { cwd: sb.project });
      const got: Outcome = ts.ok
        ? { code: 0, stdout: `${toJqJson(ts.steps, false)}\n`, stderr: [] }
        : { code: ts.exitCode, stdout: "", stderr: [ts.message] };
      expect(got).toEqual(bashOutcome(bash));
    });
  }
});

const HEADER =
  "# Some Plan — Plan\n\n**Date:** 2026-06-17   **Status:** Draft   **Spec:** docs/toolu/specs/x.md   **Topic:** Harden the thing\n\n## Context\n\nProse below the header.\n";

/** [doc body or null, field] */
const FIELDS: Record<string, [string | null, string]> = {
  "mid-line": [HEADER, "Status"],
  "multi-space gap": [HEADER, "Spec"],
  "trailing field": [HEADER, "Topic"],
  "missing field": [HEADER, "Author"],
  "single-space packing": [
    "# T\n\n**Status:** Approved **Spec:** none **Topic:** terse\n",
    "Status",
  ],
  "absent doc": [null, "Status"],
  "empty field name": [HEADER, ""],
  "first matching line wins": ["**Status:** Draft\n**Status:** Approved\n", "Status"],
  "last occurrence on a line": ["**Status:** a **Status:** b **Spec:** c\n", "Status"],
  "value with inner stars": ["**Spec:** a**b:**c   **Topic:** t\n", "Spec"],
  "tabs and CR": ["**Status:**\tApproved\t\r\n", "Status"],
  "field only in prose": ["Some prose mentions **Status:** Done in a sentence.\n", "Status"],
  "empty value": ["**Status:**   **Spec:** x\n", "Status"],
};

describe("docField vs pl_doc_field", () => {
  for (const [name, [body, field]] of Object.entries(FIELDS)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      if (body !== null) sb.write("doc.md", body);
      const doc = sb.path("doc.md");
      const bash = await bashFn(PARSE, "pl_doc_field", [doc, field], { cwd: sb.project });
      expect(bash.exitCode).toBe(0);
      expect(docField(doc, field)).toBe(bash.stdout.replace(/\n+$/, ""));
    });
  }
});

const SPEC_TWO =
  "# Some Spec\n\n## Problem\n\nMentions **AC-1:** in prose.\n\n## Acceptance criteria\n\n- **AC-1:** First.\n- **AC-2:** Second.\n- **AC-1:** Duplicate.\n\n### Sub-heading keeps the section\n\n- **AC-3:** Third, **AC-4:** same line.\n\n## Open Questions\n\n- **AC-9:** Not an AC.\n\n## Acceptance criteria\n\n- **AC-10:** Re-armed.\n";

const SPECS: Record<string, string | null> = {
  "two ACs with noise": SPEC_TWO,
  "no AC section": "# Spec\n\n## Problem\n\n- **AC-1:** stray.\n",
  "missing spec": null,
  "heading, no ids": "## Acceptance criteria\n\nnothing\n",
};

describe("parseAcs vs pl_parse_acs", () => {
  for (const [name, body] of Object.entries(SPECS)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      if (body !== null) sb.write("spec.md", body);
      const spec = sb.path("spec.md");
      const bash = await bashFn(PARSE, "pl_parse_acs", [spec], { cwd: sb.project });
      expect(bash.exitCode).toBe(0);
      const want = bash.stdout === "" ? [] : bash.stdout.replace(/\n$/, "").split("\n");
      expect(parseAcs(spec)).toEqual(want);
    });
  }
});

const refsPlan = (refs: string): string =>
  fence(
    `[{ "id": "s1", "title": "t", "check": "true", "ac_refs": ${refs} }, { "id": "s2", "title": "t", "check": "true" }]`,
  );

/** [plan body, spec argument: a SPECS key, "none", or "" for omitted] */
const REFS: Record<string, [string, string]> = {
  "all resolve": [refsPlan('["AC-1", "AC-2"]'), "two ACs with noise"],
  dangling: [refsPlan('["AC-1", "AC-9", "AC-10", "AC-7"]'), "two ACs with noise"],
  "spec none": [refsPlan('["AC-9"]'), "none"],
  "spec NONE": [refsPlan('["AC-9"]'), "NONE"],
  "spec omitted": [refsPlan('["AC-9"]'), ""],
  "spec without AC section": [refsPlan('["AC-1"]'), "no AC section"],
  "missing spec file": [refsPlan('["AC-1"]'), "missing spec"],
  "no refs": [fence('[{ "id": "s1", "title": "t", "check": "true" }]'), "two ACs with noise"],
  "odd ref values": [refsPlan('["AC-2", 3, "", "ac-1", "AC-2"]'), "two ACs with noise"],
  "only an empty ref": [refsPlan('[""]'), "two ACs with noise"],
  "unparseable plan": ["# nothing\n", "two ACs with noise"],
};

describe("checkAcRefs vs pl_check_ac_refs", () => {
  for (const [name, [plan, specKey]] of Object.entries(REFS)) {
    test.concurrent(name, async () => {
      using sb = createSandbox();
      sb.write("plan.md", plan);
      const body = SPECS[specKey];
      if (body !== undefined && body !== null) sb.write("spec.md", body);
      const spec = SPECS[specKey] === undefined ? specKey : "spec.md";
      const env = { LC_ALL: "C" };
      const args = spec === "" ? ["plan.md"] : ["plan.md", spec];
      const bash = await bashFn(PARSE, "pl_check_ac_refs", args, { cwd: sb.project, env });
      const ts = checkAcRefs("plan.md", spec, { cwd: sb.project });
      const got: Outcome = {
        code: ts.ok ? 0 : 1,
        stdout: ts.dangling.map((d) => `${d}\n`).join(""),
        stderr: ts.message === undefined ? [] : [ts.message],
      };
      expect(got).toEqual(bashOutcome(bash));
    });
  }
});

test("the step-model allow-list equals TOOLU_MODEL_ALIASES and the parser's literal", async () => {
  const res = await run(
    ["bash", "-c", '. "$1/config.sh"; printf %s "$TOOLU_MODEL_ALIASES"', "_", LIB],
    {},
  );
  expect(MODEL_ALIASES.join(" ")).toBe(res.stdout);
  const src = await Bun.file(`${LIB}/plan-ledger-parse.sh`).text();
  expect(src).toContain(`[${MODEL_ALIASES.map((m) => `"${m}"`).join(",")}]`);
});
