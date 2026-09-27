/** Per-host CLI args: approval bypass, model/effort, resume, skill syntax. */

import { describe, expect, test } from "bun:test";
import { HOST_LIMIT, agentArgs, hostKind, skillRef } from "../hosts.ts";

const base = { key: "toolu-12", bypass: true, permissionMode: "auto", resume: false };

describe("agentArgs", () => {
  test("claude: skip permissions, model, effort, session name", () => {
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

  test("codex: bypass approvals and sandbox; effort through -c", () => {
    expect(agentArgs("codex", { ...base, model: "gpt-6-sol", effort: "medium" })).toEqual([
      "--dangerously-bypass-approvals-and-sandbox",
      "--model",
      "gpt-6-sol",
      "-c",
      "model_reasoning_effort=medium",
    ]);
  });

  test("cursor: yolo, trusted workspace, MCP approval; effort lives in the model id", () => {
    expect(agentArgs("cursor", { ...base, model: "gpt-5.6-sol-high", effort: "high" })).toEqual([
      "--yolo",
      "--trust",
      "--approve-mcps",
      "--model",
      "gpt-5.6-sol-high",
    ]);
  });

  test("opencode: auto-approve and provider/model#variant", () => {
    expect(agentArgs("opencode", { ...base, model: "anthropic/claude-sonnet-5#high" })).toEqual([
      "--auto",
      "--model",
      "anthropic/claude-sonnet-5#high",
    ]);
  });

  test("resume: codex uses the resume subcommand, the rest a flag", () => {
    expect(agentArgs("codex", { ...base, resume: true }).slice(0, 2)).toEqual(["resume", "--last"]);
    expect(agentArgs("claude", { ...base, resume: true }).at(-1)).toBe("--continue");
    expect(agentArgs("cursor", { ...base, resume: true }).at(-1)).toBe("--continue");
    expect(agentArgs("opencode", { ...base, resume: true }).at(-1)).toBe("--continue");
  });

  test("safe mode keeps prompts on", () => {
    expect(agentArgs("claude", { ...base, bypass: false }).slice(0, 2)).toEqual([
      "--permission-mode",
      "auto",
    ]);
    expect(agentArgs("codex", { ...base, bypass: false })).toContain("on-request");
  });

  test("rejects args the pane shell would mangle", () => {
    expect(() => agentArgs("cursor", { ...base, model: "claude[effort=high]" })).toThrow("unsafe");
    expect(() => agentArgs("claude", { ...base, model: "a b" })).toThrow("unsafe");
  });
});

test("host aliases", () => {
  expect(hostKind("cursor-agent")).toBe("cursor");
  expect(hostKind("Claude-Code")).toBe("claude");
  expect(() => hostKind("gemini")).toThrow("unknown host");
});

test("skill syntax per host", () => {
  expect(skillRef("claude", "delivery-flow", "delivery-flow")).toBe(
    "`/delivery-flow:delivery-flow`",
  );
  expect(skillRef("codex", "pr-babysit", "babysit")).toBe("`$pr-babysit:babysit`");
  expect(skillRef("opencode", "pr-babysit", "babysit")).toBe("the `pr-babysit--babysit` skill");
});

test("limit pattern matches provider throttle messages, not ordinary output", () => {
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
