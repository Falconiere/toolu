import { expect, test } from "bun:test";
import { chmodSync, existsSync, readFileSync } from "node:fs";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { debugJson, runHost, toolStates, withServe } from "../host-run.ts";
import { ContractError } from "../schema.ts";
import { openSession } from "../session.ts";

// A real executable stands in for the opencode binary, so these tests drive the
// actual spawn, parse and process-group cleanup paths without a network (#335).

const FAKE_HOST = `#!/bin/sh
case "$1" in
  run)
    echo "not json"
    echo '{"type":"tool_use","part":{"tool":"bash","state":{"status":"error","error":"denied"}}}'
    echo '{"type":"text","part":{"text":"PROBE-DONE"}}'
    ;;
  debug)
    [ "$2" = "fail" ] && { echo "boom" >&2; exit 3; }
    echo '{"ok":true}'
    ;;
  serve)
    [ -n "$SERVE_EXIT" ] && exit "$SERVE_EXIT"
    echo $$ > "$HOME/serve.pid"
    echo "opencode server listening on http://127.0.0.1:4555"
    exec sleep 300
    ;;
esac
`;

function fakeHost(sb: Sandbox, body = FAKE_HOST): string {
  const bin = sb.write("bin/opencode", body);
  chmodSync(bin, 0o755);
  return bin;
}

/** The value a promise rejects with; a fulfilled promise yields null. */
function rejection(promise: Promise<unknown>): Promise<unknown> {
  return promise.then(
    () => null,
    (err: unknown) => err,
  );
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return !readFileSync(`/proc/${pid}/stat`, "utf8").split(" ")[2]?.startsWith("Z");
  } catch {
    return false;
  }
}

async function gone(pid: number, tries = 40): Promise<boolean> {
  if (!alive(pid)) return true;
  if (tries === 0) return false;
  await Bun.sleep(50);
  return gone(pid, tries - 1);
}

test.concurrent("runHost keeps the JSON event lines and toolStates reads their tool parts", async () => {
  using tools = createSandbox();
  using session = openSession(tools.root);
  const run = await runHost(fakeHost(tools), session, ["PROBE:x"]);
  expect(run.exitCode).toBe(0);
  expect(run.events).toEqual([
    { type: "tool_use", part: { tool: "bash", state: { status: "error", error: "denied" } } },
    { type: "text", part: { text: "PROBE-DONE" } },
  ]);
  expect(toolStates(run)).toEqual([{ tool: "bash", status: "error", error: "denied" }]);
});

test.concurrent("debugJson parses stdout and turns a failing debug command into a contract error", async () => {
  using tools = createSandbox();
  using session = openSession(tools.root);
  const bin = fakeHost(tools);
  expect(await debugJson(bin, session, ["config"])).toEqual({ ok: true });
  const err = await rejection(debugJson(bin, session, ["fail"]));
  expect(err).toBeInstanceOf(ContractError);
  expect(err instanceof Error ? err.message : "").toBe("opencode debug fail failed (exit 3): boom");
});

test.concurrent("withServe hands over the listening address, then kills the server's process group", async () => {
  using tools = createSandbox();
  using session = openSession(tools.root);
  const url = await withServe(fakeHost(tools), session, (address) => Promise.resolve(address));
  expect(url).toBe("http://127.0.0.1:4555");
  const pidFile = `${session.sb.home}/serve.pid`;
  expect(existsSync(pidFile)).toBe(true);
  expect(await gone(Number(readFileSync(pidFile, "utf8").trim()))).toBe(true);
});

test.concurrent("withServe fails with the exit code when the server dies before listening", async () => {
  using tools = createSandbox();
  using session = openSession(tools.root);
  const bin = fakeHost(
    tools,
    FAKE_HOST.replace('[ -n "$SERVE_EXIT" ] && exit "$SERVE_EXIT"', "exit 7"),
  );
  const err = await rejection(withServe(bin, session, () => Promise.resolve("unreachable")));
  expect(err).toBeInstanceOf(ContractError);
  expect(err instanceof Error ? err.message : "").toBe("opencode serve exited 7: ");
});
