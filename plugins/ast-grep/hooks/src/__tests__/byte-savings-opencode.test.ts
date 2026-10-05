/**
 * #347: on OpenCode, an ast-grep run's post result carries the session's
 * byte-savings report; every other host stays silent. The modules are the
 * ones the real `register.js` publishes, dispatched by the core post-tool
 * dispatcher over a real ledger and real ast-grep output.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { z } from "zod";
import { dispatchPostTool } from "@toolu/core/dispatch";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { PLUGIN_ROOT, REPO_ROOT } from "./golden-sandbox.ts";

const SESSION = "ses_opencode-1";
const LEDGER = "toolu/byte-savings/sesopencode-1.jsonl";
const PATTERN = "export function $N($$$) { $$$ }";
const SEARCH = `ast-grep run -p '${PATTERN}' -l typescript src`;
const Output = z.object({
  hookSpecificOutput: z.object({ additionalContext: z.string() }).optional(),
  systemMessage: z.string().optional(),
});

type Host = "opencode" | "claude";

function env(sb: Sandbox, host: Host): Record<string, string> {
  return {
    HOME: sb.home,
    PATH: process.env.PATH ?? "",
    TOOLU_CONFIG_DIR: sb.path("data"),
    TOOLU_HOST_OVERRIDE: host,
  };
}

async function project(): Promise<Sandbox> {
  const sb = createSandbox({ git: true });
  sb.write("src/app.ts", "export function greet(name: string) {\n  return name;\n}\n");
  const registered = await run(entryArgv("ast-grep", "register", PLUGIN_ROOT), {
    cwd: sb.project,
    env: env(sb, "opencode"),
    stdin: "{}",
  });
  expect(registered.exitCode).toBe(0);
  return sb;
}

/** Real ast-grep output for the skill's search example. */
async function searchOutput(sb: Sandbox): Promise<string> {
  const res = await run(["ast-grep", "run", "-p", PATTERN, "-l", "typescript", "src"], {
    cwd: sb.project,
  });
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("greet");
  return res.stdout;
}

async function post(sb: Sandbox, host: Host, payload: Record<string, unknown>): Promise<string> {
  const res = await dispatchPostTool(
    JSON.stringify({
      session_id: SESSION,
      cwd: sb.project,
      hook_event_name: "PostToolUse",
      ...payload,
    }),
    {
      builtins: [],
      libDir: join(REPO_ROOT, "plugins/toolu/hooks/lib"),
      cwd: sb.project,
      env: env(sb, host),
      selectedRegistrySpecs: new Set(["ast-grep@toolu"]),
    },
  );
  expect(res.exitCode).toBe(0);
  if (res.stdout.trim() === "") return "";
  const doc = Output.parse(JSON.parse(res.stdout));
  return doc.hookSpecificOutput?.additionalContext ?? doc.systemMessage ?? "";
}

function astGrepCall(output: string): Record<string, unknown> {
  return {
    tool_name: "Bash",
    tool_input: { command: SEARCH },
    tool_response: { metadata: { exit_code: 0 }, interrupted: false, output },
  };
}

test("an ast-grep run on OpenCode reports the session; read results stay silent", async () => {
  using sb = await project();
  const output = await searchOutput(sb);
  const returned = Buffer.byteLength(output.replace(/\n+$/u, ""), "utf8");
  const first = await post(sb, "opencode", astGrepCall(output));
  expect(first).toBe(
    `ast-grep byte savings this session:\nast-grep: returned=${returned} (n=1)\nTOTAL returned: ${returned} bytes (~${Math.floor(returned / 4)} tok)`,
  );
  const read = await post(sb, "opencode", {
    tool_name: "Read",
    tool_input: { file_path: "src/app.ts" },
    tool_response: { metadata: {}, interrupted: false, output: "00001| export function greet" },
  });
  expect(read).toBe("");
  const second = await post(sb, "opencode", astGrepCall(output));
  expect(second).toContain(`ast-grep: returned=${returned * 2} (n=2)`);
  expect(second).toContain("read: returned=28 full=");
  expect(
    readFileSync(sb.path(`data/${LEDGER}`), "utf8")
      .trim()
      .split("\n"),
  ).toHaveLength(3);
});

test("the same ast-grep run under Claude Code records the ledger and says nothing", async () => {
  using sb = await project();
  const output = await searchOutput(sb);
  expect(await post(sb, "claude", astGrepCall(output))).toBe("");
  expect(readFileSync(sb.path(`data/${LEDGER}`), "utf8")).toBe(
    `{"kind":"ast-grep","returned":${Buffer.byteLength(output.trimEnd(), "utf8")},"full":0}\n`,
  );
});

test("a corrupt ledger keeps the record and says the report is unavailable", async () => {
  using sb = await project();
  sb.write(`data/${LEDGER}`, "not json\n");
  const output = await searchOutput(sb);
  const message = await post(sb, "opencode", astGrepCall(output));
  expect(message).toBe(
    `ast-grep byte savings this session:\nreport unavailable: ${sb.path(`data/${LEDGER}`)}:1: invalid ledger line`,
  );
  expect(readFileSync(sb.path(`data/${LEDGER}`), "utf8").split("\n")[1]).toStartWith(
    '{"kind":"ast-grep"',
  );
});

test("empty ast-grep output records nothing and reports nothing", async () => {
  using sb = await project();
  expect(await post(sb, "opencode", astGrepCall(""))).toBe("");
  expect(() => readFileSync(sb.path(`data/${LEDGER}`), "utf8")).toThrow();
});
