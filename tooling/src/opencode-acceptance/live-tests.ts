/**
 * Acceptance checks over the `*.live.test.ts` files (#362). Each file runs in a
 * `bun test` subprocess with `TOOLU_LIVE_OPENCODE=1` and a JUnit report, so a
 * skipped host test is visible and fails the check instead of passing quietly.
 * Tests that call a real external service run apart, only with their own flag,
 * and count toward live availability, never toward host acceptance.
 */
import { existsSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run, type EnvPatch } from "@toolu/conformance/harness/spawn";
import { ROOT } from "../opencode-host/scenarios-entry.ts";
import { hostEvidence, type AcceptanceCheck, type CheckOutcome, type Service } from "./checks.ts";

type ExternalTest = {
  id: string;
  /** The exact `test(...)` name in the file. */
  name: string;
  /** The env flag that un-skips it. */
  flag: string;
  service: string;
  /** An env key it cannot run without; absent means `not-configured`. */
  requires?: string;
};
type LiveTestFile = {
  id: string;
  file: string;
  plugins: readonly string[];
  service: Service;
  external?: readonly ExternalTest[];
};
type JUnitCase = { name: string; status: "passed" | "failed" | "skipped"; seconds: number };
type ExternalStatus = "available" | "unavailable" | "not-configured";
export type ExternalResult = {
  id: string;
  service: string;
  execution: "in-process";
  status: ExternalStatus;
  detail: string;
};

const DIR = "tools/toolu-opencode/src/plugin/__tests__";
const TEST_TIMEOUT_MS = 900_000;
const RUN_TIMEOUT_MS = 1_200_000;

export const LIVE_TEST_FILES: readonly LiveTestFile[] = [
  {
    id: "live.context-delivery",
    file: `${DIR}/context-delivery.live.test.ts`,
    plugins: ["toolu"],
    service: "none",
  },
  {
    id: "live.context7",
    file: `${DIR}/context7-delivery.live.test.ts`,
    plugins: ["context7"],
    service: "fixture",
    external: [
      {
        id: "external.context7",
        name: "one real context7.com search runs through the OpenCode-published command",
        flag: "TOOLU_LIVE_CONTEXT7",
        service: "context7.com",
      },
    ],
  },
  {
    id: "live.core-workflows",
    file: `${DIR}/core-workflows.live.test.ts`,
    plugins: ["toolu", "toolu-review"],
    service: "fixture",
  },
  {
    id: "live.delivery-workflows",
    file: `${DIR}/delivery-workflows.live.test.ts`,
    plugins: ["brainstorm", "delivery-flow"],
    service: "fixture",
  },
  {
    id: "live.epic-worker",
    file: `${DIR}/epic-worker.live.test.ts`,
    plugins: ["epic-orchestrator"],
    service: "fixture",
  },
  {
    id: "live.jev",
    file: `${DIR}/jev-delivery.live.test.ts`,
    plugins: ["jev"],
    service: "fixture",
    external: [
      {
        id: "external.jev",
        name: "one real TypeSafe judgment runs through the OpenCode-published wrapper",
        flag: "TOOLU_LIVE_JEV",
        service: "api.typesafe.ai",
        requires: "TYPESAFE_API_KEY",
      },
    ],
  },
  {
    id: "live.jira",
    file: `${DIR}/jira-delivery.live.test.ts`,
    plugins: ["jira"],
    service: "fixture",
  },
];

const ENTITIES: Record<string, string> = {
  "&quot;": '"',
  "&apos;": "'",
  "&lt;": "<",
  "&gt;": ">",
  "&amp;": "&",
};

function attribute(attrs: string, name: string): string {
  const raw = attrs.match(new RegExp(`\\s${name}="([^"]*)"`))?.[1] ?? "";
  return raw.replace(/&(quot|apos|lt|gt|amp);/g, (entity) => ENTITIES[entity] ?? entity);
}

/** The test cases of a `bun test --reporter=junit` report. */
export function parseJUnit(xml: string): JUnitCase[] {
  return [...xml.matchAll(/<testcase\b([^>]*?)(?:\/>|>([\s\S]*?)<\/testcase>)/g)].map((match) => {
    const attrs = match[1] ?? "";
    const body = match[2] ?? "";
    const status = body.includes("<skipped")
      ? "skipped"
      : /<(failure|error)\b/.test(body)
        ? "failed"
        : "passed";
    return {
      name: attribute(attrs, "name"),
      status,
      seconds: Number(attribute(attrs, "time")) || 0,
    };
  });
}

/** The variables a live test subprocess needs; the harness strips every other `TOOLU_*`. */
function forwarded(): EnvPatch {
  const keys = ["TOOLU_ACCEPTANCE_PACKAGE", "TOOLU_OPENCODE_HOST_BIN", "TOOLU_OPENCODE_HOST_CACHE"];
  return Object.fromEntries(keys.map((key) => [key, process.env[key]]));
}

/** Run one live test file and parse its JUnit report. */
export async function runLiveFile(
  file: string,
  env: EnvPatch,
): Promise<{ cases: JUnitCase[]; exitCode: number; tail: string }> {
  const dir = mkdtempSync(join(tmpdir(), "toolu-acceptance-junit-"));
  const out = join(dir, "junit.xml");
  try {
    const argv = [
      process.execPath,
      "test",
      "--timeout",
      String(TEST_TIMEOUT_MS),
      "--reporter=junit",
    ];
    const res = await run([...argv, `--reporter-outfile=${out}`, file], {
      cwd: ROOT,
      env: { ...forwarded(), ...env },
      timeoutMs: RUN_TIMEOUT_MS,
    });
    const cases = existsSync(out) ? parseJUnit(readFileSync(out, "utf8")) : [];
    return { cases, exitCode: res.timedOut ? 124 : res.exitCode, tail: res.stderr.slice(-1500) };
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Host tests must all pass and the external ones must stay skipped without their flag. */
async function runHostTests(entry: LiveTestFile): Promise<CheckOutcome> {
  const externalNames = new Set((entry.external ?? []).map((test) => test.name));
  const { cases, exitCode, tail } = await runLiveFile(join(ROOT, entry.file), {
    TOOLU_LIVE_OPENCODE: "1",
  });
  const host = cases.filter((test) => !externalNames.has(test.name));
  const external = cases.filter((test) => externalNames.has(test.name));
  const pass =
    exitCode === 0 &&
    host.length > 0 &&
    host.every((test) => test.status === "passed") &&
    external.length === externalNames.size &&
    external.every((test) => test.status === "skipped");
  const listed = cases.map((test) => `${test.name}: ${test.status}`).join("; ");
  return { pass, observed: { exitCode, cases: listed, ...(pass ? {} : { stderr: tail }) } };
}

export function liveTestChecks(
  files: readonly LiveTestFile[] = LIVE_TEST_FILES,
): AcceptanceCheck[] {
  return files.map((entry) => ({
    id: entry.id,
    family: "live",
    plugins: entry.plugins,
    evidence: hostEvidence(entry.service),
    run: () => runHostTests(entry),
  }));
}

/** One external test, run on its own with only its flag; `keys` holds the credentials the run set aside. */
export async function runExternal(
  entry: LiveTestFile,
  test: ExternalTest,
  keys: Readonly<Record<string, string>>,
): Promise<ExternalResult> {
  const base: Pick<ExternalResult, "id" | "service" | "execution"> = {
    id: test.id,
    service: test.service,
    execution: "in-process",
  };
  const key = test.requires === undefined ? undefined : keys[test.requires];
  if (test.requires !== undefined && key === undefined) {
    return { ...base, status: "not-configured", detail: `${test.requires} is not set` };
  }
  const env: EnvPatch = {
    [test.flag]: "1",
    ...(test.requires === undefined ? {} : { [test.requires]: key }),
  };
  const { cases, tail } = await runLiveFile(join(ROOT, entry.file), env);
  const found = cases.find((item) => item.name === test.name);
  const status: ExternalStatus = found?.status === "passed" ? "available" : "unavailable";
  return {
    ...base,
    status,
    detail: found === undefined ? `not run: ${tail}` : `${found.status} in ${found.seconds}s`,
  };
}
