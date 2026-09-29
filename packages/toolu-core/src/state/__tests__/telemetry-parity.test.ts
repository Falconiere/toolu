/**
 * `telemetryAppend` vs bash `telemetry_append` (#255). In one real repo, bash
 * appends first and TypeScript second. The two lines must match byte for
 * byte (after normalizing `t`), land in the same file, and parse as v1 lines.
 * The opt-outs write nothing in either implementation. Rejected extras
 * (secret-shaped keys, nested payloads) write nothing in TypeScript.
 */
import { describe, expect, test } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { TelemetryLineSchema, type TelemetryEvent, type TelemetryExtras } from "../state-schema.ts";
import { telemetryAppend } from "../telemetry.ts";

const TELEMETRY_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/telemetry.sh",
);
const STAMP = /"t":"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ"/;

type Host = "claude" | "codex";

function repo(host: Host = "claude"): Sandbox {
  const sb = createSandbox({ git: true, branch: "feat/255-state" });
  sb.writeConfig(host, "project", { version: 1 });
  return sb;
}

function envFor(
  sb: Sandbox,
  host: Host,
  extra: Record<string, string> = {},
): Record<string, string> {
  return { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: host, ...extra };
}

async function bashAppend(
  sb: Sandbox,
  host: Host,
  event: string,
  extras: string,
  extra = {},
): Promise<string> {
  const res = await run(
    [
      "bash",
      "-c",
      '. "$1"; telemetry_append "$2" "$3" "$4"; echo "rc=$?"',
      "_",
      TELEMETRY_SH,
      sb.project,
      event,
      extras,
    ],
    { cwd: sb.project, env: envFor(sb, host, extra) },
  );
  expect(res.stdout.trim()).toBe("rc=0");
  return res.stderr;
}

function tsAppend<E extends TelemetryEvent>(
  sb: Sandbox,
  host: Host,
  event: E,
  extras: TelemetryExtras[E],
  extra = {},
) {
  const warnings: string[] = [];
  const result = telemetryAppend(sb.project, event, extras, {
    env: envFor(sb, host, extra),
    host,
    warn: (m) => warnings.push(m),
  });
  return { result, warnings };
}

/** Every `*.jsonl` under `dir`, as relative path → content. */
function logs(dir: string): Record<string, string> {
  if (!existsSync(dir)) return {};
  return Object.fromEntries(
    readdirSync(dir).map((name) => [name, readFileSync(join(dir, name), "utf8")]),
  );
}

const EVENTS: { [E in TelemetryEvent]: TelemetryExtras[E] } = {
  gate_fail: { file: "/repo/src/a\u007f é.ts", source: "ts-quality-hook" },
  gate_clear: { file: "/repo/src/a.ts", source: "ts-quality-hook" },
  step_run: { step_id: "S1", status: "green", exit_code: 0, duration_s: 12, attempt: 2 },
  ac_coverage: { covered: 7, uncovered: 2 },
  docs_attested: { decision: "updated" },
  docs_nudge: {},
  push_check: { result: "deny", reason_code: "stale-review", round: null },
  delegation: {
    model: "sonnet",
    subagent_type: "Explore",
    reasoning_effort: null,
    step_id: null,
    step_model: null,
  },
};

describe("parity per event", () => {
  for (const host of ["claude", "codex"] as const) {
    for (const event of Object.keys(EVENTS) as TelemetryEvent[]) {
      test(`${host}: ${event}`, async () => {
        using sb = repo(host);
        await bashAppend(sb, host, event, JSON.stringify(EVENTS[event]));
        const { result } = tsAppend(sb, host, event, EVENTS[event]);
        const dir = join(sb.project, `.${host}`, "tmp", "telemetry");
        expect(result).toEqual({ written: true, file: join(dir, "feat_255-state.jsonl") });
        const files = logs(dir);
        expect(Object.keys(files)).toEqual(["feat_255-state.jsonl"]);
        const [bashLine, tsLine, rest] = (files["feat_255-state.jsonl"] ?? "").split("\n");
        expect(rest).toBe("");
        expect(tsLine?.replace(STAMP, '"t":"T"')).toBe(bashLine?.replace(STAMP, '"t":"T"'));
        expect(TelemetryLineSchema.safeParse(JSON.parse(bashLine ?? "")).success).toBe(true);
      });
    }
  }
});

test("TELEMETRY_DIR overrides the directory in both", async () => {
  using sb = repo();
  const dir = join(sb.root, "custom-telemetry");
  await bashAppend(sb, "claude", "docs_nudge", "{}", { TELEMETRY_DIR: dir });
  const { result } = tsAppend(sb, "claude", "docs_nudge", {}, { TELEMETRY_DIR: dir });
  expect(result).toEqual({ written: true, file: join(dir, "feat_255-state.jsonl") });
  expect((logs(dir)["feat_255-state.jsonl"] ?? "").split("\n")).toHaveLength(3);
});

describe("opt-outs write nothing in either implementation", () => {
  test("telemetry.enabled false", async () => {
    using sb = repo();
    sb.writeConfig("claude", "project", { version: 1, telemetry: { enabled: false } });
    await bashAppend(sb, "claude", "docs_nudge", "{}");
    expect(tsAppend(sb, "claude", "docs_nudge", {}).result).toEqual({
      written: false,
      reason: "disabled",
    });
    expect(logs(join(sb.project, ".claude", "tmp", "telemetry"))).toEqual({});
  });

  test("detached HEAD", async () => {
    using sb = repo();
    sb.git("checkout", "-q", "--detach");
    await bashAppend(sb, "claude", "docs_nudge", "{}");
    expect(tsAppend(sb, "claude", "docs_nudge", {}).result).toEqual({
      written: false,
      reason: "no branch",
    });
    expect(logs(join(sb.project, ".claude", "tmp", "telemetry"))).toEqual({});
  });

  test("a line over 3900 bytes", async () => {
    using sb = repo();
    const extras = { file: "x".repeat(4000), source: "s" };
    const stderr = await bashAppend(sb, "claude", "gate_fail", JSON.stringify(extras));
    expect(stderr).toContain("(>3900); skipping append");
    const { result, warnings } = tsAppend(sb, "claude", "gate_fail", extras);
    expect(result.written).toBe(false);
    expect(warnings[0]).toMatch(
      /^telemetry: assembled line for event "gate_fail" is \d+ bytes \(>3900\); skipping append$/,
    );
    expect(logs(join(sb.project, ".claude", "tmp", "telemetry"))).toEqual({});
  });
});

describe("no secret logging", () => {
  const SECRET = "ghp_S3CRETtoken";
  const rejected: [string, TelemetryEvent, unknown][] = [
    ["a command key", "docs_nudge", { command: `curl -H "Authorization: ${SECRET}"` }],
    ["a token key", "gate_fail", { file: "a", source: "b", token: SECRET }],
    [
      "a tool_input payload",
      "delegation",
      {
        model: null,
        subagent_type: null,
        reasoning_effort: null,
        step_id: null,
        step_model: null,
        tool_input: { prompt: SECRET },
      },
    ],
    ["a nested value", "docs_attested", { decision: { secret: SECRET } }],
    ["a smuggled protocol key", "docs_nudge", { branch: SECRET }],
  ];
  for (const [name, event, extras] of rejected) {
    test(`rejects ${name}`, () => {
      using sb = repo();
      const warnings: string[] = [];
      // Runtime callers are untyped; hand the payload through as JSON would arrive.
      const payload: TelemetryExtras[typeof event] = JSON.parse(JSON.stringify(extras));
      const result = telemetryAppend(sb.project, event, payload, {
        env: envFor(sb, "claude"),
        host: "claude",
        warn: (m) => warnings.push(m),
      });
      expect(result.written).toBe(false);
      expect(warnings).toEqual([`telemetry: invalid extras for event "${event}"; skipping append`]);
      const all = Object.values(logs(join(sb.project, ".claude", "tmp", "telemetry"))).join("");
      expect(all).not.toContain(SECRET);
    });
  }

  test("rejects an unknown event name", () => {
    using sb = repo();
    const warnings: string[] = [];
    const event: TelemetryEvent = JSON.parse('"shell_command"');
    const result = telemetryAppend(
      sb.project,
      event,
      {},
      { env: envFor(sb, "claude"), host: "claude", warn: (m) => warnings.push(m) },
    );
    expect(result.written).toBe(false);
    expect(warnings).toEqual(['telemetry: unknown event "shell_command"; skipping append']);
  });
});
