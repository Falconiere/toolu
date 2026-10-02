/**
 * Verifies docs/portable-core.md against the #205 contract checklist: required
 * headings, the documented OpenCode pins and contract link (#335), citations,
 * the four classification tokens in the Policy split section (and no invalid
 * `maybe-later`), and no citation of the superseded V2 plugin docs.
 * `PORTABLE_CORE_DOC` points it at another copy.
 */
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { envOr } from "./env.ts";
import { V2_DOCS } from "./opencode-host/contract-check.ts";
import { contractPaths } from "./opencode-host/results.ts";
import { ContractError, PinSchema, readJson } from "./opencode-host/schema.ts";

const ROOT = resolve(import.meta.dir, "../..");
const FIXTURE = resolve(ROOT, "tooling/fixtures/portable-core/protected-files-pre.json");

const HEADINGS = [
  "## Pins",
  "## Package boundaries",
  "## Zod boundary rules",
  "## Decision contract",
  "## Event vocabulary",
  "## OpenCode dispatch contract",
  "## Policy split",
  "## Protected-files gate trace",
  "## OpenCode interception",
  "### Capability-results",
  "## Release blockers",
];

const CITATIONS: ReadonlyArray<readonly [string, string]> = [
  ["opencode-host-contract.md", "missing host contract link"],
  ["dispatchPreTool", "missing native dispatch contract"],
  ["gates/protected-files.ts", "missing protected-files gate citation"],
  ["gate-mode.sh", "missing gate-mode.sh citation"],
  ["dispatch.sh", "missing dispatch.sh citation"],
  ["tooling/fixtures/portable-core/protected-files-pre.json", "missing fixture citation"],
  ["deny", "missing deny mapping language"],
];

const TOKENS = ["shell-out", "port-native", "port-new", "no-map"];

class DocError extends Error {}

/** Lines after `## Policy split` up to the next `## ` heading. */
function policySection(doc: string): string {
  const out: string[] = [];
  let inside = false;
  for (const line of doc.split("\n")) {
    if (line.startsWith("## Policy split")) inside = true;
    else if (line.startsWith("## ")) inside = false;
    else if (inside) out.push(line);
  }
  return out.join("\n");
}

function check(docPath: string): void {
  if (!existsSync(docPath)) throw new DocError(`missing ${docPath}`);
  if (!existsSync(FIXTURE)) throw new DocError(`missing ${FIXTURE}`);
  const doc = readFileSync(docPath, "utf8");
  for (const heading of HEADINGS) {
    if (!doc.includes(heading)) throw new DocError(`missing heading: ${heading}`);
  }
  const pin = readJson(contractPaths().pin, PinSchema);
  const pins: ReadonlyArray<readonly [string, string]> = [
    [
      `${pin.cli.package}@${pin.cli.version}`,
      `missing CLI pin ${pin.cli.package}@${pin.cli.version}`,
    ],
    [`${pin.sdk.package}@${pin.sdk.version}`, "missing SDK pin"],
  ];
  for (const [needle, message] of [...pins, ...CITATIONS]) {
    if (!doc.includes(needle)) throw new DocError(message);
  }
  const policy = policySection(doc);
  if (policy === "") throw new DocError("empty Policy split section");
  for (const token of TOKENS) {
    if (!policy.includes(token)) throw new DocError(`missing classification token: ${token}`);
  }
  if (
    policy
      .split("\n")
      .some((line) => line.includes("`maybe-later`") || /^\s*-\s*maybe-later/.test(line))
  ) {
    throw new DocError("invalid classification token maybe-later");
  }
  if (doc.includes(V2_DOCS)) throw new DocError(`cites the V2 contract (${V2_DOCS})`);
}

function main(): number {
  const docPath = envOr("PORTABLE_CORE_DOC", resolve(ROOT, "docs/portable-core.md"));
  try {
    check(docPath);
  } catch (err: unknown) {
    if (!(err instanceof DocError || err instanceof ContractError)) throw err;
    console.error(`check-portable-core-doc: ${err.message}`);
    return 1;
  }
  process.stdout.write("check-portable-core-doc: ok\n");
  return 0;
}

if (import.meta.main) process.exitCode = main();
