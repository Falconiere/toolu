/** Pinned-host browser startup, discovery, diagnostics and lifecycle (#346). */
import { randomUUID } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, readlinkSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { runHost, toolStates } from "./host-run.ts";
import { SELECTION, install, selection, skills, toolResults } from "./install-host.ts";
import {
  diagnostics,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { messagesText } from "./scenario.ts";
import { ContractError } from "./schema.ts";
import type { ProbeSession } from "./session.ts";

const SKILL = "agent-browser-agent-browser";
const HELPER = ".opencode/toolu/state/agent-browser/agent-browser.sh";
const COMMAND = '"$TOOLU_CONFIG_DIR/agent-browser/agent-browser.sh"';

function browserFiles(): Record<string, string> {
  return { [SELECTION]: selection(["agent-browser"]) };
}

function helperIsPublished(project: string): boolean {
  const path = join(project, HELPER);
  if (!existsSync(path)) return false;
  return readlinkSync(path).endsWith("/plugins/agent-browser/hooks/dist/agent-browser.js");
}

const Doctor = z.object({
  checks: z.array(z.object({ id: z.string(), status: z.string(), message: z.string() })),
});
const Snapshot = z.object({
  data: z.object({ refs: z.record(z.string(), z.object({ role: z.string(), name: z.string() })) }),
});

/** Use an installed real Chromium while the browser profile stays inside the isolated HOME. */
async function browserPrerequisites(): Promise<{ binary: string; chrome: string }> {
  const binary = Bun.which("agent-browser");
  if (binary === null) throw new ContractError("browser.workflow requires agent-browser on PATH");
  const doctor = await run([binary, "doctor", "--json"], { timeoutMs: 30_000 });
  if (doctor.exitCode !== 0)
    throw new ContractError(`agent-browser doctor failed: ${doctor.stderr.trim()}`);
  const parsed = Doctor.parse(JSON.parse(doctor.stdout));
  const installed = parsed.checks.find((check) => check.id === "chrome.installed");
  const chrome = installed?.message.split(" at ").at(-1);
  if (installed?.status !== "pass" || chrome === undefined || !existsSync(chrome)) {
    throw new ContractError("browser.workflow requires Chromium; run agent-browser install");
  }
  return { binary, chrome };
}

async function enabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = { "browser.enabled": [{ tool: "skill", args: { name: SKILL } }] };
  using s = install(ctx, browserFiles(), scripts, () => ({ permission: { bash: "allow" } }));
  const discovered = await skills(ctx, s);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:browser.enabled"]);
  const loaded = toolResults(hostRun.events).find((result) => result.tool === "skill");
  const system = messagesText(s, "system");
  const helper = join(s.sb.project, HELPER);
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    discovered: discovered.rows.some((row) => row.name === SKILL),
    skillStatus: loaded?.status ?? "missing",
    skillHasPath: loaded?.text.includes("agent-browser/agent-browser.sh") ?? false,
    skillHasLabel: loaded?.text.includes("# OpenCode") ?? false,
    helper: helperIsPublished(s.sb.project),
    context: system.includes(helper) && system.includes("snapshot") && system.includes(SKILL),
  };
  return {
    pass:
      observed.ready &&
      observed.discovered &&
      observed.skillStatus === "completed" &&
      observed.skillHasPath &&
      observed.skillHasLabel &&
      observed.helper &&
      observed.context,
    observed,
  };
}

async function missingBinary(ctx: EntryContext): Promise<EntryResult> {
  const command = `AGENT_BROWSER_BIN=/nonexistent/agent-browser ${COMMAND} snapshot; code=$?; echo "browser_exit=$code"`;
  const scripts = {
    "browser.missing-binary": [{ tool: "bash", args: { command, description: "missing CLI" } }],
  };
  using s = install(ctx, browserFiles(), scripts, () => ({ permission: { bash: "allow" } }));
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:browser.missing-binary"]);
  const bash = toolResults(hostRun.events).find((result) => result.tool === "bash");
  const text = bash?.text ?? "";
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    helper: helperIsPublished(s.sb.project),
    called: bash?.status === "completed",
    exit127: text.includes("browser_exit=127"),
    diagnostic: text.includes("agent-browser not found") && text.includes("agent-browser install"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function missingChromium(ctx: EntryContext): Promise<EntryResult> {
  const binary = Bun.which("agent-browser");
  if (binary === null)
    throw new ContractError("browser.missing-chromium requires agent-browser on PATH");
  const socketDir = mkdtempSync(join(tmpdir(), "ab-"));
  try {
    const command = `AGENT_BROWSER_EXECUTABLE_PATH=/nonexistent/chrome AGENT_BROWSER_SESSION=missing ${COMMAND} open about:blank; code=$?; echo "browser_exit=$code"`;
    const scripts = {
      "browser.missing-chromium": [
        { tool: "bash", args: { command, description: "missing Chromium" } },
      ],
    };
    using s = install(ctx, browserFiles(), scripts, () => ({ permission: { bash: "allow" } }));
    s.env.AGENT_BROWSER_BIN = binary;
    s.env.AGENT_BROWSER_SOCKET_DIR = socketDir;
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:browser.missing-chromium"]);
    const bash = toolResults(hostRun.events).find((result) => result.tool === "bash");
    const text = bash?.text ?? "";
    const observed = {
      ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
      called: bash?.status === "completed",
      failed: /browser_exit=[1-9]/.test(text),
      diagnostic:
        /\/nonexistent\/chrome|executable/i.test(text) && !/socket path|session name/i.test(text),
    };
    const pass = Object.values(observed).every(Boolean);
    return { pass, observed: pass ? observed : { ...observed, failure: text.slice(0, 500) } };
  } finally {
    rmSync(socketDir, { recursive: true, force: true });
  }
}

async function disabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = {
    "browser.disabled": [
      { tool: "bash", args: { command: "printf 'toolu-ready'", description: "ready" } },
    ],
  };
  using s = install(ctx, browserFiles(), scripts, () => ({ permission: { bash: "allow" } }));
  const first = await skills(ctx, s);
  const helper = join(s.sb.project, HELPER);
  const firstHelper = helperIsPublished(s.sb.project);
  s.sb.write(SELECTION, selection(["toolu"]));
  const second = await skills(ctx, s);
  const previousRequests = s.requests().length;
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:browser.disabled"]);
  const newRequests = s.requests().slice(previousRequests);
  const bash = toolStates(hostRun).find((result) => result.tool === "bash");
  const observed = {
    firstSkill: first.rows.some((row) => row.name === SKILL),
    firstHelper,
    skillRemoved: !second.rows.some((row) => row.name === SKILL),
    helperRemoved: !existsSync(helper),
    contextRemoved: !JSON.stringify(newRequests).includes("agent-browser is ready at"),
    coreReady: diagnostics(hostRun.stderr, "toolu: ready") === 1 && bash?.status === "completed",
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

function projectText(session: ProbeSession, file: string): string {
  return session.exists(file) ? readFileSync(join(session.sb.project, file), "utf8") : "";
}

function workflowResult(
  hostRun: Awaited<ReturnType<typeof runHost>>,
  s: ProbeSession,
): EntryResult {
  const results = toolResults(hostRun.events);
  const skill = results.find((result) => result.tool === "skill");
  const bash = results.find((result) => result.tool === "bash");
  const before = projectText(s, "before.json");
  const after = projectText(s, "after.json");
  const status = projectText(s, "status.json");
  const snapshot = Snapshot.safeParse(before === "" ? null : JSON.parse(before));
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    skill: skill?.status === "completed",
    bash: bash?.status === "completed",
    beforeRef: snapshot.success && snapshot.data.data.refs.e1?.role === "button",
    resnapshot: after.length > 0,
    changedText: status.includes("Done"),
    modelResult: bash?.text.includes("Done") ?? false,
    helper: helperIsPublished(s.sb.project),
  };
  const pass = Object.values(observed).every(Boolean);
  return {
    pass,
    observed: pass
      ? observed
      : { ...observed, shell: (bash?.text ?? "").slice(0, 500), before: before.slice(0, 500) },
  };
}

async function workflow(ctx: EntryContext): Promise<EntryResult> {
  const { binary, chrome } = await browserPrerequisites();
  const socketDir = mkdtempSync(join(tmpdir(), "ab-"));
  const page = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch: () =>
      new Response(
        '<!doctype html><title>Browser probe</title><button id="action" onclick="document.getElementById(\'status\').textContent=\'Done\'">Run</button><p id="status">Waiting</p>',
        { headers: { "content-type": "text/html" } },
      ),
  });
  try {
    const command = [
      `trap '${COMMAND} close >/dev/null 2>&1' EXIT`,
      `${COMMAND} open http://127.0.0.1:${page.port}/`,
      `${COMMAND} snapshot > before.json`,
      `${COMMAND} click @e1`,
      `${COMMAND} snapshot > after.json`,
      `${COMMAND} get text '#status' > status.json`,
      `${COMMAND} close`,
      "cat status.json",
    ].join(" && ");
    const scripts = {
      "browser.workflow": [
        { tool: "skill", args: { name: SKILL } },
        { tool: "bash", args: { command, description: "real browser workflow" } },
      ],
    };
    using s = install(ctx, browserFiles(), scripts, () => ({ permission: { bash: "allow" } }));
    const session = `t${randomUUID().slice(0, 8)}`;
    s.env.AGENT_BROWSER_BIN = binary;
    s.env.AGENT_BROWSER_EXECUTABLE_PATH = chrome;
    s.env.AGENT_BROWSER_SESSION = session;
    s.env.AGENT_BROWSER_NAMESPACE = session;
    s.env.AGENT_BROWSER_SOCKET_DIR = socketDir;
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:browser.workflow"]);
    return workflowResult(hostRun, s);
  } finally {
    await page.stop(true);
    rmSync(socketDir, { recursive: true, force: true });
  }
}

export const BROWSER_SCENARIOS: EntryScenario[] = [
  {
    id: "browser.enabled",
    claim: "Native skill and project helper path reach the OpenCode model",
    run: enabled,
  },
  {
    id: "browser.missing-binary",
    claim: "An absent external CLI gives the model an install diagnostic",
    run: missingBinary,
  },
  {
    id: "browser.missing-chromium",
    claim: "An absent Chromium executable gives the model a launch diagnostic",
    run: missingChromium,
  },
  {
    id: "browser.disabled",
    claim: "Disabling agent-browser removes its skill, helper and context",
    run: disabled,
  },
  {
    id: "browser.workflow",
    claim: "OpenCode drives a real Chromium through the published helper and snapshot refs",
    run: workflow,
  },
];
