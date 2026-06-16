import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  agentDir,
  baseEnv,
  gateFile,
  gateStatusText,
  isPostToolTarget,
  mappedToolName,
  parseHookOutput,
  readGateStatus,
  runHook,
  toolInputForEvent,
  toolCallPayload,
  toolResultPayload,
} from "../toolu.ts";

describe("mappedToolName", () => {
  test("maps known opencode tools to their Claude Code counterparts", () => {
    expect(mappedToolName("bash")).toBe("Bash");
    expect(mappedToolName("read")).toBe("Read");
    expect(mappedToolName("edit")).toBe("Edit");
    expect(mappedToolName("patch")).toBe("Edit");
    expect(mappedToolName("write")).toBe("Write");
    expect(mappedToolName("grep")).toBe("Grep");
    expect(mappedToolName("glob")).toBe("Glob");
    expect(mappedToolName("list")).toBe("Glob");
    expect(mappedToolName("webfetch")).toBe("WebFetch");
    expect(mappedToolName("websearch")).toBe("WebSearch");
  });

  test("returns undefined for unknown tools", () => {
    expect(mappedToolName("unknown")).toBeUndefined();
    expect(mappedToolName("skill")).toBeUndefined();
    expect(mappedToolName("task")).toBeUndefined();
  });
});

describe("isPostToolTarget", () => {
  test("includes bash, edit, write, patch", () => {
    expect(isPostToolTarget("bash")).toBe(true);
    expect(isPostToolTarget("edit")).toBe(true);
    expect(isPostToolTarget("write")).toBe(true);
    expect(isPostToolTarget("patch")).toBe(true);
  });

  test("excludes read, grep, glob, list, webfetch, websearch", () => {
    expect(isPostToolTarget("read")).toBe(false);
    expect(isPostToolTarget("grep")).toBe(false);
    expect(isPostToolTarget("glob")).toBe(false);
    expect(isPostToolTarget("list")).toBe(false);
    expect(isPostToolTarget("webfetch")).toBe(false);
    expect(isPostToolTarget("websearch")).toBe(false);
  });
});

describe("toolInputForEvent", () => {
  test("renames opencode filePath to Claude Code file_path for edit", () => {
    const result = toolInputForEvent({ tool: "edit" }, { filePath: "/tmp/foo.ts", oldString: "a", newString: "b" });
    expect(result.file_path).toBe("/tmp/foo.ts");
    expect(result.path).toBe("/tmp/foo.ts");
    expect(result.target_file).toBe("/tmp/foo.ts");
    expect(result.old_string).toBe("a");
    expect(result.new_string).toBe("b");
  });

  test("renames opencode filePath to file_path for write", () => {
    const result = toolInputForEvent({ tool: "write" }, { filePath: "/tmp/foo.ts", content: "hello" });
    expect(result.file_path).toBe("/tmp/foo.ts");
    expect(result.content).toBe("hello");
  });

  test("renames opencode filePath to file_path for read", () => {
    const result = toolInputForEvent({ tool: "read" }, { filePath: "/tmp/foo.ts", offset: 10, limit: 50 });
    expect(result.file_path).toBe("/tmp/foo.ts");
    expect(result.offset).toBe(10);
    expect(result.limit).toBe(50);
  });

  test("passes bash command through unchanged", () => {
    const result = toolInputForEvent({ tool: "bash" }, { command: "rm -rf /", timeout: 5000 });
    expect(result.command).toBe("rm -rf /");
    expect(result.timeout).toBe(5000);
  });

  test("passes grep args through unchanged", () => {
    const result = toolInputForEvent({ tool: "grep" }, { pattern: "TODO", path: "src/" });
    expect(result.pattern).toBe("TODO");
    expect(result.path).toBe("src/");
  });

  test("falls back to path when filePath absent", () => {
    const result = toolInputForEvent({ tool: "read" }, { path: "/tmp/bar.ts" });
    expect(result.file_path).toBe("/tmp/bar.ts");
  });
});

describe("toolCallPayload", () => {
  test("emits {tool_name, tool_input} shape expected by toolu hook scripts", () => {
    const payload = toolCallPayload(
      { tool: "edit" },
      { filePath: "/tmp/foo.ts", oldString: "a", newString: "b" },
    );
    const parsed = JSON.parse(payload);
    expect(parsed.tool_name).toBe("Edit");
    expect(parsed.tool_input.file_path).toBe("/tmp/foo.ts");
  });

  test("emits undefined tool_name for unmapped tools", () => {
    const payload = toolCallPayload({ tool: "skill" }, { name: "x" });
    const parsed = JSON.parse(payload);
    expect(parsed.tool_name).toBeUndefined();
  });
});

describe("toolResultPayload", () => {
  test("emits isError flag in both tool_response and tool_output", () => {
    const payload = toolResultPayload(
      { tool: "bash" },
      { command: "exit 1" },
      "Command exited with code 1",
      true,
    );
    const parsed = JSON.parse(payload);
    expect(parsed.tool_name).toBe("Bash");
    expect(parsed.tool_input.command).toBe("exit 1");
    expect(parsed.tool_response.is_error).toBe(true);
    expect(parsed.tool_output.isError).toBe(true);
  });

  test("emits isError=false for successful runs", () => {
    const payload = toolResultPayload({ tool: "edit" }, { filePath: "/x" }, "ok", false);
    const parsed = JSON.parse(payload);
    expect(parsed.tool_response.is_error).toBe(false);
    expect(parsed.tool_output.isError).toBe(false);
  });
});

describe("parseHookOutput", () => {
  test("returns undefined for empty stdout", () => {
    expect(parseHookOutput("")).toBeUndefined();
  });

  test("returns undefined for non-JSON stdout", () => {
    expect(parseHookOutput("not json at all")).toBeUndefined();
  });

  test("parses permissionDecision: deny", () => {
    const out = parseHookOutput(JSON.stringify({
      hookSpecificOutput: { permissionDecision: "deny", permissionDecisionReason: "no" },
    }));
    expect(out?.hookSpecificOutput?.permissionDecision).toBe("deny");
    expect(out?.hookSpecificOutput?.permissionDecisionReason).toBe("no");
  });

  test("parses additionalContext advisory", () => {
    const out = parseHookOutput(JSON.stringify({
      hookSpecificOutput: { additionalContext: "remember to test" },
    }));
    expect(out?.hookSpecificOutput?.additionalContext).toBe("remember to test");
  });

  test("parses top-level decision: block (post-tool)", () => {
    const out = parseHookOutput(JSON.stringify({ decision: "block", reason: "no good" }));
    expect(out?.decision).toBe("block");
    expect(out?.reason).toBe("no good");
  });
});

describe("agentDir", () => {
  const original = process.env.OPENCODE_CONFIG_DIR;
  afterEach(() => {
    if (original === undefined) delete process.env.OPENCODE_CONFIG_DIR;
    else process.env.OPENCODE_CONFIG_DIR = original;
  });

  test("respects OPENCODE_CONFIG_DIR override", () => {
    process.env.OPENCODE_CONFIG_DIR = "/custom/opencode";
    expect(agentDir()).toBe("/custom/opencode");
  });

  test("falls back to ~/.config/opencode", () => {
    delete process.env.OPENCODE_CONFIG_DIR;
    expect(agentDir()).toMatch(/\.config\/opencode$/);
  });
});

describe("baseEnv", () => {
  test("sets TOOLU_RUNTIME=opencode and TOOLU_CONFIG_DIR", () => {
    const env = baseEnv("/tmp");
    expect(env.TOOLU_RUNTIME).toBe("opencode");
    expect(env.TOOLU_CONFIG_DIR).toBeDefined();
    expect(env.TOOLU_PROJECT_CONFIG_DIRNAME).toBe(".opencode");
    expect(env.OPENCODE_CONFIG_DIR).toBeDefined();
  });
});

describe("gateFile", () => {
  test("constructs path under project root's .opencode/tmp/", () => {
    const result = gateFile("/some/path");
    expect(result).toMatch(/\.opencode\/tmp\/quality-gate-status\.json$/);
  });
});

describe("gateStatusText + readGateStatus", () => {
  let dir: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "toolu-test-"));
  });
  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  test("returns 'gate: clear' when no gate file exists", () => {
    expect(gateStatusText(dir)).toBe("gate: clear");
    expect(readGateStatus(dir)).toBeUndefined();
  });

  test("returns 'gate: failing — <reason>' when status is failing", () => {
    const gateDir = join(dir, ".opencode", "tmp");
    require("node:fs").mkdirSync(gateDir, { recursive: true });
    writeFileSync(
      join(gateDir, "quality-gate-status.json"),
      JSON.stringify({ status: "failing", reason: "ts-quality: lint errors" }),
    );
    expect(gateStatusText(dir)).toBe("gate: failing — ts-quality: lint errors");
  });

  test("returns 'gate: clear' when status field is missing", () => {
    const gateDir = join(dir, ".opencode", "tmp");
    require("node:fs").mkdirSync(gateDir, { recursive: true });
    writeFileSync(join(gateDir, "quality-gate-status.json"), "{}");
    expect(gateStatusText(dir)).toBe("gate: clear");
  });
});

describe("runHook", () => {
  test("resolves (not hangs) when the script does not exist (ENOENT)", async () => {
    // Regression for review comment #3423919709: when spawn fails, Node fires
    // `error` but not `close`. The old handler only appended to stderr and
    // never resolved the promise, so the opencode session hung forever.
    // A 100ms deadline is a generous bound — pre-fix this would time out.
    const result = await Promise.race([
      runHook("/this/path/does/not/exist.sh", "/tmp", "{}"),
      new Promise<{ code: number; stdout: string; stderr: string }>((_, reject) =>
        setTimeout(() => reject(new Error("runHook hung past 100ms")), 100),
      ),
    ]);
    expect(result.code).toBe(127);
    expect(result.stderr.toLowerCase()).toMatch(/enoent|no such file/);
  });

  test("resolves with a script that exits non-zero", async () => {
    const tmpScript = join(tmpdir(), `runhook-${Date.now()}.sh`);
    writeFileSync(tmpScript, "#!/usr/bin/env bash\nexit 42\n");
    try {
      const r = await runHook(tmpScript, "/tmp", "{}");
      expect(r.code).toBe(42);
    } finally {
      rmSync(tmpScript, { force: true });
    }
  });
});
