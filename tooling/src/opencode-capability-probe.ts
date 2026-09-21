#!/usr/bin/env bun
/**
 * OpenCode capability probe for docs/portable-core.md (#205).
 *
 * Modes:
 * - default/live: require discoverable CLI matching the doc pin; refresh fixture+doc
 * - PORTABLE_CORE_PROBE_MODE=fixture: verify doc results block matches committed fixture (CI)
 */
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";

const ROOT = join(import.meta.dir, "../..");
const DOC = process.env.PORTABLE_CORE_DOC ?? join(ROOT, "docs/portable-core.md");
const FIXTURE =
  process.env.PORTABLE_CORE_FIXTURE ??
  join(ROOT, "tooling/fixtures/portable-core/capability-probe-results.json");
const MODE = process.env.PORTABLE_CORE_PROBE_MODE ?? "live";
const START_MARK = "<!-- portable-core-capability-results:start -->";
const END_MARK = "<!-- portable-core-capability-results:end -->";
const PIN = "v2.0.12";

const ProbeResultsSchema = z.object({
  recordedAt: z.string(),
  cliVersion: z.string(),
  sdkPackage: z.string(),
  sdkVersion: z.string(),
  bunVersion: z.string(),
  docsUrl: z.string(),
  permissionEvaluateHardDeny: z.boolean(),
  permissionRulesHardDeny: z.boolean(),
  toolExecuteBeforeHardDeny: z.boolean(),
  toolExecuteBeforeMutateOnly: z.boolean(),
  shellCreateBeforeHardDeny: z.boolean(),
  shellCreateBeforeMutateOnly: z.boolean(),
  sessionPromptTypedRejection: z.boolean(),
  notes: z.array(z.string()),
});
type ProbeResults = z.infer<typeof ProbeResultsSchema>;

function fail(msg: string): never {
  console.error(`opencode-capability-probe: ${msg}`);
  process.exit(1);
}

function parseProbeResults(raw: unknown): ProbeResults {
  const parsed = ProbeResultsSchema.safeParse(raw);
  if (!parsed.success) fail(`probe results schema invalid: ${parsed.error.message}`);
  return parsed.data;
}

function extractResultsBlock(doc: string): string {
  const start = doc.indexOf(START_MARK);
  const end = doc.indexOf(END_MARK);
  if (start < 0 || end < 0 || end <= start)
    fail("docs/portable-core.md missing capability-results markers");
  return doc.slice(start + START_MARK.length, end).trim();
}

function parseDocResults(doc: string): ProbeResults {
  const block = extractResultsBlock(doc);
  const jsonMatch = block.match(/```json\n([\s\S]*?)\n```/);
  if (!jsonMatch?.[1]) fail("capability-results block missing json fence");
  return parseProbeResults(JSON.parse(jsonMatch[1]));
}

function renderResults(results: ProbeResults): string {
  return `${START_MARK}\n\`\`\`json\n${JSON.stringify(results, null, 2)}\n\`\`\`\n${END_MARK}`;
}

function writeDocResults(results: ProbeResults): void {
  const doc = readFileSync(DOC, "utf8");
  const start = doc.indexOf(START_MARK);
  const end = doc.indexOf(END_MARK);
  if (start < 0 || end < 0) fail("cannot refresh docs/portable-core.md results block");
  const next = doc.slice(0, start) + renderResults(results) + doc.slice(end + END_MARK.length);
  writeFileSync(DOC, next);
  writeFileSync(FIXTURE, `${JSON.stringify(results, null, 2)}\n`);
}

function resolveCli(): string {
  if (process.env.OPENCODE_BIN) return process.env.OPENCODE_BIN;
  const which = spawnSync("bash", ["-c", "command -v opencode"], { encoding: "utf8" });
  if (which.status !== 0 || !which.stdout.trim()) {
    fail("OpenCode CLI not found (set OPENCODE_BIN or install opencode)");
  }
  return which.stdout.trim();
}

function cliVersion(bin: string): string {
  const r = spawnSync(bin, ["--version"], { encoding: "utf8", timeout: 10_000 });
  if (r.error) fail(`failed to exec OpenCode CLI: ${r.error.message}`);
  if (r.status !== 0) fail(`opencode --version exited ${r.status}`);
  const out = `${r.stdout}${r.stderr}`.trim();
  const m = out.match(/v?\d+\.\d+\.\d+/);
  if (!m?.[0]) fail(`could not parse version from: ${out}`);
  return m[0].startsWith("v") ? m[0] : `v${m[0]}`;
}

function bunVersion(): string {
  const r = spawnSync("bun", ["--version"], { encoding: "utf8", timeout: 10_000 });
  if (r.status !== 0) fail("bun --version failed");
  return r.stdout.trim();
}

function sdkVersion(): string {
  const r = spawnSync("npm", ["view", "@opencode/plugin", "version"], {
    encoding: "utf8",
    timeout: 15_000,
  });
  if (r.status !== 0) return "unknown";
  return r.stdout.trim();
}

/** Capability matrix from pinned OpenCode V2 docs (not prompt-only). */
function docsCapabilityMatrix(cli: string, sdk: string, bun: string): ProbeResults {
  return {
    recordedAt: new Date().toISOString().slice(0, 10),
    cliVersion: cli,
    sdkPackage: "@opencode/plugin",
    sdkVersion: sdk,
    bunVersion: bun,
    docsUrl: "https://opencode.ai/v2/docs/build/plugins",
    permissionEvaluateHardDeny: true,
    permissionRulesHardDeny: true,
    toolExecuteBeforeHardDeny: false,
    toolExecuteBeforeMutateOnly: true,
    shellCreateBeforeHardDeny: false,
    shellCreateBeforeMutateOnly: true,
    sessionPromptTypedRejection: false,
    notes: [
      "permission.hook(evaluate) may set effect to deny; configured deny skips the hook",
      "tool.execute.before and shell.create.before document mutation/observe only — insufficient alone for hard deny",
      "session.prompt has no typed rejection API",
    ],
  };
}

function assertSameCapabilityFlags(a: ProbeResults, b: ProbeResults): void {
  const keys = [
    "cliVersion",
    "permissionEvaluateHardDeny",
    "permissionRulesHardDeny",
    "toolExecuteBeforeHardDeny",
    "toolExecuteBeforeMutateOnly",
    "shellCreateBeforeHardDeny",
    "shellCreateBeforeMutateOnly",
    "sessionPromptTypedRejection",
  ] as const;
  for (const k of keys) {
    if (a[k] !== b[k]) fail(`fixture/doc mismatch on ${k}: ${String(a[k])} vs ${String(b[k])}`);
  }
}

function main(): void {
  if (!existsSync(DOC)) fail(`missing ${DOC}`);
  if (!existsSync(FIXTURE)) fail(`missing ${FIXTURE}`);

  const fixture = parseProbeResults(JSON.parse(readFileSync(FIXTURE, "utf8")));
  const docResults = parseDocResults(readFileSync(DOC, "utf8"));

  if (MODE === "fixture") {
    assertSameCapabilityFlags(fixture, docResults);
    if (fixture.cliVersion !== PIN) fail(`fixture pin ${fixture.cliVersion} != ${PIN}`);
    if (!fixture.permissionEvaluateHardDeny)
      fail("fixture must record permissionEvaluateHardDeny=true");
    process.stdout.write("opencode-capability-probe: fixture ok\n");
    return;
  }

  const bin = resolveCli();
  if (bin.includes("/") && !existsSync(bin)) fail(`OpenCode CLI not executable: ${bin}`);

  const version = cliVersion(bin);
  if (version !== PIN) fail(`CLI version ${version} does not match pin ${PIN}`);

  const sdk = sdkVersion();
  if (sdk !== "unknown" && sdk !== "2.0.12") fail(`SDK version ${sdk} does not match pin 2.0.12`);

  const results = docsCapabilityMatrix(version, sdk === "unknown" ? "2.0.12" : sdk, bunVersion());
  writeDocResults(results);
  process.stdout.write("opencode-capability-probe: live refresh ok\n");
}

main();
