/** Per-host CLI args: approval bypass, model/effort, resume, skill syntax. */

import { expect, test } from "bun:test";
import { HOST_LIMIT, agentArgs, hostKind, parseHostKind, skillRef } from "../hosts.ts";

const base = { key: "toolu-12", bypass: true, permissionMode: "auto", resume: false };

test.concurrent("agentArgs claude: skip permissions, model, effort, session name", () => {
  expect(agentArgs("claude", { ...base, model: "opus", effort: "high" })).toEqual([
    "--dangerously-skip-permissions",
    "-n",
    "toolu-12",
    "--model",
    "opus",
    "--effort",
    "high",
  ]);
});

test.concurrent("agentArgs codex: bypass approvals and sandbox; effort through -c", () => {
  expect(agentArgs("codex", { ...base, model: "gpt-6-sol", effort: "medium" })).toEqual([
    "--no-daemon",
    "--dangerously-bypass-approvals-and-sandbox",
    "--model",
    "gpt-6-sol",
    "-c",
    "model_reasoning_effort=medium",
  ]);
});

test.concurrent("agentArgs cursor: yolo, trusted workspace, MCP approval; effort lives in the model id", () => {
  expect(agentArgs("cursor", { ...base, model: "gpt-5.6-sol-high", effort: "high" })).toEqual([
    "--yolo",
    "--trust",
    "--approve-mcps",
    "--model",
    "gpt-5.6-sol-high",
  ]);
});

test.concurrent("agentArgs opencode: auto-approve and provider/model#variant", () => {
  expect(agentArgs("opencode", { ...base, model: "anthropic/claude-sonnet-5#high" })).toEqual([
    "--standalone",
    "--auto",
    "--model",
    "anthropic/claude-sonnet-5#high",
  ]);
});

test.concurrent("captured sessions resume exactly and OpenCode owns its standalone runtime", () => {
  expect(
    agentArgs("codex", { ...base, resume: true, sessionId: "01a-session" }).slice(0, 2),
  ).toEqual(["resume", "01a-session"]);
  expect(
    agentArgs("claude", { ...base, resume: true, sessionId: "abc-session" }).slice(0, 2),
  ).toEqual(["--resume", "abc-session"]);
  expect(
    agentArgs("opencode", { ...base, resume: true, sessionId: "ses_123" }).slice(0, 3),
  ).toEqual(["--standalone", "--session", "ses_123"]);
});

test.concurrent("agentArgs resume: codex uses the resume subcommand, the rest a flag", () => {
  expect(agentArgs("codex", { ...base, resume: true }).slice(0, 2)).toEqual(["resume", "--last"]);
  expect(agentArgs("claude", { ...base, resume: true }).at(-1)).toBe("--continue");
  expect(agentArgs("cursor", { ...base, resume: true }).at(-1)).toBe("--continue");
  expect(agentArgs("opencode", { ...base, resume: true }).at(-1)).toBe("--continue");
});

test.concurrent("agentArgs safe mode keeps prompts on", () => {
  expect(agentArgs("claude", { ...base, bypass: false }).slice(0, 2)).toEqual([
    "--permission-mode",
    "auto",
  ]);
  expect(agentArgs("codex", { ...base, bypass: false })).toContain("on-request");
});

test.concurrent("agentArgs rejects args the pane shell would mangle", () => {
  expect(() => agentArgs("cursor", { ...base, model: "claude[effort=high]" })).toThrow("unsafe");
  expect(() => agentArgs("claude", { ...base, model: "a b" })).toThrow("unsafe");
});

test.concurrent("host aliases", () => {
  expect(hostKind("cursor-agent")).toBe("cursor");
  expect(hostKind("Claude-Code")).toBe("claude");
  expect(() => hostKind("gemini")).toThrow("unknown host");
  expect(parseHostKind(" Cursor-Agent ")).toBe("cursor");
  expect(parseHostKind("gemini")).toBeNull();
});

test.concurrent("skill syntax per host", () => {
  expect(skillRef("claude", "delivery-flow", "delivery-flow")).toBe(
    "`/delivery-flow:delivery-flow`",
  );
  expect(skillRef("codex", "pr-babysit", "babysit")).toBe("`$pr-babysit:babysit`");
  expect(skillRef("opencode", "pr-babysit", "babysit")).toBe("the `pr-babysit--babysit` skill");
});

test.concurrent("limit pattern matches provider throttle messages, not ordinary output", () => {
  for (const line of [
    "You've hit your usage limit. Upgrade or try again at 5:00 PM.",
    "Claude usage limit reached. Your limit will reset at 3pm",
    "Error: 429 Too Many Requests",
    "stream error: rate limited, retrying",
  ]) {
    expect(HOST_LIMIT.test(line)).toBe(true);
  }
  expect(HOST_LIMIT.test("All 34 tests passed")).toBe(false);
});
