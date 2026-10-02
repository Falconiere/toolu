import { expect, test } from "bun:test";
import { join } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { TOUCH_TOOL, TouchArgs } from "../mcp-server.ts";

// Real stdio subprocess for the MCP fixture the live OpenCode probes use (#335).

const SERVER = join(import.meta.dir, "../mcp-server.ts");

function lines(...messages: unknown[]): string {
  return messages.map((m) => (typeof m === "string" ? m : JSON.stringify(m))).join("\n") + "\n";
}

test.concurrent("initialize, list and call append the marker and answer each request", async () => {
  using sb = createSandbox();
  const marker = sb.path("marker.txt");
  const res = await run([process.execPath, SERVER], {
    env: { MCP_MARKER: marker },
    stdin: lines(
      { jsonrpc: "2.0", id: 1, method: "initialize", params: { protocolVersion: "2025-06-18" } },
      { jsonrpc: "2.0", method: "notifications/initialized" },
      { jsonrpc: "2.0", id: 2, method: "tools/list" },
      {
        jsonrpc: "2.0",
        id: 3,
        method: "tools/call",
        params: { name: "touch", arguments: { name: "allowed" } },
      },
    ),
  });
  expect(res.exitCode).toBe(0);
  const replies: unknown[] = res.stdout
    .trim()
    .split("\n")
    .map((l): unknown => JSON.parse(l));
  expect(replies).toEqual([
    {
      jsonrpc: "2.0",
      id: 1,
      result: {
        protocolVersion: "2025-06-18",
        capabilities: { tools: {} },
        serverInfo: { name: "toolu-probe", version: "1.0.0" },
      },
    },
    { jsonrpc: "2.0", id: 2, result: { tools: [TOUCH_TOOL] } },
    { jsonrpc: "2.0", id: 3, result: { content: [{ type: "text", text: "touched allowed" }] } },
  ]);
  expect(await Bun.file(marker).text()).toBe("allowed\n");
});

test.concurrent("malformed lines, unknown methods and bad arguments return errors without side effects", async () => {
  using sb = createSandbox();
  const marker = sb.path("marker.txt");
  const res = await run([process.execPath, SERVER], {
    env: { MCP_MARKER: marker },
    stdin: lines(
      "{not json",
      { jsonrpc: "2.0", id: 4, method: "resources/list" },
      { jsonrpc: "2.0", id: 5, method: "tools/call", params: { name: "touch", arguments: {} } },
      {
        jsonrpc: "2.0",
        id: 6,
        method: "tools/call",
        params: { name: "rm", arguments: { name: "x" } },
      },
    ),
  });
  expect(res.exitCode).toBe(0);
  const replies: unknown[] = res.stdout
    .trim()
    .split("\n")
    .map((l): unknown => JSON.parse(l));
  expect(replies).toEqual([
    { jsonrpc: "2.0", id: null, error: { code: -32700, message: "parse error" } },
    { jsonrpc: "2.0", id: 4, error: { code: -32601, message: "method not found: resources/list" } },
    {
      jsonrpc: "2.0",
      id: 5,
      error: { code: -32602, message: TouchArgs.safeParse({}).error?.message },
    },
    { jsonrpc: "2.0", id: 6, error: { code: -32602, message: "unknown tool" } },
  ]);
  expect(await Bun.file(marker).exists()).toBe(false);
});

test.concurrent("a call without MCP_MARKER is a JSON-RPC error, not a tool result", async () => {
  const res = await run([process.execPath, SERVER], {
    env: { MCP_MARKER: undefined },
    stdin: lines({
      jsonrpc: "2.0",
      id: 7,
      method: "tools/call",
      params: { name: "touch", arguments: { name: "x" } },
    }),
  });
  expect(res.exitCode).toBe(0);
  expect(JSON.parse(res.stdout.trim())).toEqual({
    jsonrpc: "2.0",
    id: 7,
    error: { code: -32603, message: "MCP_MARKER is not set" },
  });
});
