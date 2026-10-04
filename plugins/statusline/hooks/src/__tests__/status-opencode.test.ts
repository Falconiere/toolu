/**
 * The OpenCode status report (#359): the real status bundle reads the adapter's
 * status record from `TOOLU_CONFIG_DIR`, the data root OpenCode's bash gets,
 * and turns each state into a line with a next step. stdout is only the
 * report and stderr stays empty (`report` asserts both).
 */
import { expect, test } from "bun:test";
import { realpathSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { put, report, repo } from "./harness.ts";

const WRITTEN = "2026-10-04T12:00:00.000Z";

function dataRoot(sb: Sandbox): string {
  return join(sb.project, ".opencode/toolu/state");
}

function recordAt(sb: Sandbox, body: string): string {
  const path = join(dataRoot(sb), "toolu/opencode-status.json");
  put(path, body);
  return path;
}

function opencode(sb: Sandbox): Promise<string> {
  return report(sb, sb.project, {
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: dataRoot(sb),
  });
}

test.concurrent("status: OpenCode lists the plugins toolu started and its readiness", async () => {
  using sb = createSandbox();
  repo(sb.project);
  const path = recordAt(
    sb,
    JSON.stringify({
      version: 1,
      written: WRITTEN,
      project: sb.project,
      status: "ready",
      selection: "project",
      plugins: [
        { name: "statusline", entries: ["session-start"], artifacts: 1 },
        { name: "jev", entries: ["session-start", "user-prompt-submit"], artifacts: 1 },
      ],
      notes: ['enabled plugin "nope" in plugins.json is not installed'],
    }),
  );
  sb.write(".opencode/tmp/quality-gate-status.json", '{"status":"passing"}\n');
  expect(await opencode(sb)).toBe(
    [
      "Host: OpenCode",
      "toolu: ready — 2 plugins (project selection), 2 startup artifacts",
      "Plugins: statusline (session-start), jev (session-start, user-prompt-submit)",
      'Startup notes: enabled plugin "nope" in plugins.json is not installed',
      `Startup record: ${path}, written ${WRITTEN} for ${sb.project}`,
      `Repository: ${realpathSync(sb.project)}`,
      "Branch: main",
      "Working tree: clean",
      "Quality gate: passing",
      "",
    ].join("\n"),
  );
});

test.concurrent("status: OpenCode reports a not-ready startup with its cause", async () => {
  using sb = createSandbox();
  repo(sb.project);
  const reason = "bootstrap: jev/session-start: exited 1";
  const path = recordAt(
    sb,
    JSON.stringify({
      version: 1,
      written: WRITTEN,
      project: sb.project,
      status: "not-ready",
      reason,
      plugins: [],
      notes: [],
    }),
  );
  const out = await opencode(sb);
  expect(out).toStartWith(
    [
      "Host: OpenCode",
      `toolu: not ready — ${reason}; every tool call stays denied until OpenCode restarts with the cause fixed`,
      `Startup record: ${path}, written ${WRITTEN} for ${sb.project}`,
      `Repository: ${realpathSync(sb.project)}`,
    ].join("\n"),
  );
  expect(out).toContain("Quality gate: no recorded state\n");
});

test.concurrent("status: OpenCode names a missing record and how to create it", async () => {
  using sb = createSandbox();
  repo(sb.project);
  const path = join(dataRoot(sb), "toolu/opencode-status.json");
  const out = await opencode(sb);
  expect(out).toContain(
    `\ntoolu: no startup record at ${path} — start OpenCode with the toolu plugin in this project, then run the status skill again\n`,
  );
  expect(out).toContain("Branch: main\n");
});

test.concurrent("status: OpenCode names an unreadable record and still reports the rest", async () => {
  for (const body of ["{}", '{"version":1,"written":"x"']) {
    using sb = createSandbox();
    repo(sb.project);
    const path = recordAt(sb, body);
    const out = await opencode(sb);
    expect(out).toContain(
      `\ntoolu: unreadable startup record at ${path} — restart OpenCode to rewrite it\n`,
    );
    expect(out).toContain("Branch: main\n");
    expect(out).not.toContain("toolu: ready");
  }
});

test.concurrent("status: Codex never reads an OpenCode status record", async () => {
  using sb = createSandbox();
  repo(sb.project);
  recordAt(sb, "{}");
  const out = await report(sb, sb.project, {
    TOOLU_HOST_OVERRIDE: "codex",
    TOOLU_CONFIG_DIR: dataRoot(sb),
  });
  expect(out).not.toContain("toolu:");
  expect(out).not.toContain("Startup record");
});
