#!/usr/bin/env bun
/** Validate the committed live host-lifecycle evidence for issue #376. */

import { readFileSync } from "node:fs";
import { resolve } from "node:path";

export const PROBE_SCHEMA_VERSION = 1;
export const PROBE_CONTRACT_VERSION = 1;
export const PROBE_HOSTS = ["claude", "codex", "cursor", "opencode"] as const;
export const PROBE_CAPABILITIES = [
  "start",
  "tool_task",
  "resume",
  "background_work",
  "cancel_replace",
  "teardown",
] as const;
export const PROBE_OUTCOMES = ["verified", "failed", "unsupported", "unverified"] as const;

export type ProbeHost = (typeof PROBE_HOSTS)[number];
export type ProbeCapability = (typeof PROBE_CAPABILITIES)[number];
export type ProbeOutcome = (typeof PROBE_OUTCOMES)[number];

export type ProbeNativeError = {
  code: string;
  message: string;
  exitCode: number | null;
};

export type ProbeCapabilityEvidence = {
  outcome: ProbeOutcome;
  summary: string;
  observedAt: string;
  command: string[];
  evidence: string[];
  nativeError: ProbeNativeError | null;
};

export type ProbeHostEvidence = {
  binary: string;
  version: string | null;
  versionCommand: string[];
  profileIsolation: string;
  capabilities: Record<ProbeCapability, ProbeCapabilityEvidence>;
};

export type HostProbeEvidence = {
  schemaVersion: number;
  probeContractVersion: number;
  generatedAt: string;
  source: { issue: string; revision: string };
  environment: { platform: string; arch: string; herdrVersion: string; topology: string };
  hosts: Record<ProbeHost, ProbeHostEvidence>;
};

export const DEFAULT_EVIDENCE_PATH = resolve(
  import.meta.dir,
  "../../../docs/toolu/evidence/epic-hosts-v1.json",
);

function fail(path: string, message: string): never {
  throw new Error(`${path}: ${message}`);
}

function object(value: unknown, path: string): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    fail(path, "expected object");
  }
  return value as Record<string, unknown>;
}

function exactKeys(
  value: Record<string, unknown>,
  expected: readonly string[],
  path: string,
): void {
  const actual = Object.keys(value).sort();
  const wanted = [...expected].sort();
  if (actual.join("\0") !== wanted.join("\0")) {
    fail(path, `expected keys ${wanted.join(", ")}; got ${actual.join(", ")}`);
  }
}

function nonEmptyString(value: unknown, path: string): string {
  if (typeof value !== "string" || value.trim() === "") fail(path, "expected non-empty string");
  return value;
}

function stringArray(value: unknown, path: string): string[] {
  if (!Array.isArray(value)) fail(path, "expected string array");
  return value.map((entry, index) => nonEmptyString(entry, `${path}[${index}]`));
}

function timestamp(value: unknown, path: string): string {
  const text = nonEmptyString(value, path);
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{3})?Z$/.test(text)) {
    fail(path, "expected UTC ISO-8601 timestamp");
  }
  if (Number.isNaN(Date.parse(text))) fail(path, "invalid timestamp");
  return text;
}

function nativeError(value: unknown, path: string): ProbeNativeError | null {
  if (value === null) return null;
  const data = object(value, path);
  exactKeys(data, ["code", "message", "exitCode"], path);
  const exitCode = data["exitCode"];
  if (exitCode !== null && (!Number.isInteger(exitCode) || typeof exitCode !== "number")) {
    fail(`${path}.exitCode`, "expected integer or null");
  }
  return {
    code: nonEmptyString(data["code"], `${path}.code`),
    message: nonEmptyString(data["message"], `${path}.message`),
    exitCode,
  };
}

function capability(value: unknown, path: string): ProbeCapabilityEvidence {
  const data = object(value, path);
  exactKeys(data, ["outcome", "summary", "observedAt", "command", "evidence", "nativeError"], path);
  const outcome = nonEmptyString(data["outcome"], `${path}.outcome`);
  if (!(PROBE_OUTCOMES as readonly string[]).includes(outcome)) {
    fail(`${path}.outcome`, `expected one of ${PROBE_OUTCOMES.join(", ")}`);
  }
  const evidence = stringArray(data["evidence"], `${path}.evidence`);
  const error = nativeError(data["nativeError"], `${path}.nativeError`);
  if (outcome === "verified" && evidence.length === 0) {
    fail(`${path}.evidence`, "verified capability requires concrete evidence");
  }
  if (outcome === "failed" && error === null) {
    fail(`${path}.nativeError`, "failed capability requires the native failure");
  }
  return {
    outcome: outcome as ProbeOutcome,
    summary: nonEmptyString(data["summary"], `${path}.summary`),
    observedAt: timestamp(data["observedAt"], `${path}.observedAt`),
    command: stringArray(data["command"], `${path}.command`),
    evidence,
    nativeError: error,
  };
}

function host(value: unknown, path: string): ProbeHostEvidence {
  const data = object(value, path);
  exactKeys(
    data,
    ["binary", "version", "versionCommand", "profileIsolation", "capabilities"],
    path,
  );
  const version = data["version"];
  if (version !== null && (typeof version !== "string" || version.trim() === "")) {
    fail(`${path}.version`, "expected non-empty string or null");
  }
  const capabilitiesData = object(data["capabilities"], `${path}.capabilities`);
  exactKeys(capabilitiesData, PROBE_CAPABILITIES, `${path}.capabilities`);
  const capabilities = Object.fromEntries(
    PROBE_CAPABILITIES.map((name) => [
      name,
      capability(capabilitiesData[name], `${path}.capabilities.${name}`),
    ]),
  ) as Record<ProbeCapability, ProbeCapabilityEvidence>;
  return {
    binary: nonEmptyString(data["binary"], `${path}.binary`),
    version,
    versionCommand: stringArray(data["versionCommand"], `${path}.versionCommand`),
    profileIsolation: nonEmptyString(data["profileIsolation"], `${path}.profileIsolation`),
    capabilities,
  };
}

export function validateProbeEvidence(value: unknown): HostProbeEvidence {
  const data = object(value, "evidence");
  exactKeys(
    data,
    ["schemaVersion", "probeContractVersion", "generatedAt", "source", "environment", "hosts"],
    "evidence",
  );
  if (data["schemaVersion"] !== PROBE_SCHEMA_VERSION) {
    fail("evidence.schemaVersion", `expected ${PROBE_SCHEMA_VERSION}`);
  }
  if (data["probeContractVersion"] !== PROBE_CONTRACT_VERSION) {
    fail("evidence.probeContractVersion", `expected ${PROBE_CONTRACT_VERSION}`);
  }
  const source = object(data["source"], "evidence.source");
  exactKeys(source, ["issue", "revision"], "evidence.source");
  const issue = nonEmptyString(source["issue"], "evidence.source.issue");
  if (issue !== "https://github.com/Falconiere/toolu/issues/376") {
    fail("evidence.source.issue", "expected issue #376 URL");
  }
  const revision = nonEmptyString(source["revision"], "evidence.source.revision");
  if (!/^[0-9a-f]{40}$/.test(revision)) fail("evidence.source.revision", "expected full git SHA");
  const environment = object(data["environment"], "evidence.environment");
  exactKeys(environment, ["platform", "arch", "herdrVersion", "topology"], "evidence.environment");
  const hostsData = object(data["hosts"], "evidence.hosts");
  exactKeys(hostsData, PROBE_HOSTS, "evidence.hosts");
  const hosts = Object.fromEntries(
    PROBE_HOSTS.map((name) => [name, host(hostsData[name], `evidence.hosts.${name}`)]),
  ) as Record<ProbeHost, ProbeHostEvidence>;
  return {
    schemaVersion: PROBE_SCHEMA_VERSION,
    probeContractVersion: PROBE_CONTRACT_VERSION,
    generatedAt: timestamp(data["generatedAt"], "evidence.generatedAt"),
    source: { issue, revision },
    environment: {
      platform: nonEmptyString(environment["platform"], "evidence.environment.platform"),
      arch: nonEmptyString(environment["arch"], "evidence.environment.arch"),
      herdrVersion: nonEmptyString(
        environment["herdrVersion"],
        "evidence.environment.herdrVersion",
      ),
      topology: nonEmptyString(environment["topology"], "evidence.environment.topology"),
    },
    hosts,
  };
}

export function readProbeEvidence(path = DEFAULT_EVIDENCE_PATH): HostProbeEvidence {
  return validateProbeEvidence(JSON.parse(readFileSync(path, "utf8")) as unknown);
}

export function summarizeProbeEvidence(evidence: HostProbeEvidence): string {
  const counts = Object.fromEntries(PROBE_OUTCOMES.map((outcome) => [outcome, 0])) as Record<
    ProbeOutcome,
    number
  >;
  const gaps: string[] = [];
  for (const hostName of PROBE_HOSTS) {
    for (const capabilityName of PROBE_CAPABILITIES) {
      const outcome = evidence.hosts[hostName].capabilities[capabilityName].outcome;
      counts[outcome] += 1;
      if (outcome !== "verified") gaps.push(`${hostName}.${capabilityName}=${outcome}`);
    }
  }
  return [
    `probe-evidence: valid schema=${evidence.schemaVersion} contract=${evidence.probeContractVersion}`,
    `outcomes: ${PROBE_OUTCOMES.map((name) => `${name}=${counts[name]}`).join(" ")}`,
    `gaps: ${gaps.length === 0 ? "none" : gaps.join(", ")}`,
  ].join("\n");
}

async function main(args: string[]): Promise<number> {
  if (args[0] !== "--check-evidence" || args.length > 2) {
    process.stderr.write("usage: probe.ts --check-evidence [evidence.json]\n");
    return 2;
  }
  try {
    const evidence = readProbeEvidence(args[1] ?? DEFAULT_EVIDENCE_PATH);
    process.stdout.write(`${summarizeProbeEvidence(evidence)}\n`);
    return 0;
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : String(error);
    process.stderr.write(`probe-evidence: invalid: ${message}\n`);
    return 1;
  }
}

if (import.meta.main) process.exit(await main(process.argv.slice(2)));
