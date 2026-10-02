#!/usr/bin/env bun
/**
 * Minimal stdio MCP server for the live OpenCode host probes (#335). It
 * declares one tool, `touch {name}`, whose only effect is appending `name` to
 * the file named by `MCP_MARKER`. The marker proves whether a denied MCP call
 * reached the server. Newline-delimited JSON-RPC 2.0 on stdin/stdout.
 */
import { appendFileSync } from "node:fs";
import { z } from "zod";

const Request = z.looseObject({
  jsonrpc: z.literal("2.0"),
  id: z.union([z.string(), z.number()]).optional(),
  method: z.string(),
  params: z.unknown().optional(),
});
type Request = z.infer<typeof Request>;

export const TouchArgs = z.object({ name: z.string().min(1) });
const InitializeParams = z.looseObject({ protocolVersion: z.string() });

export const TOUCH_TOOL = {
  name: "touch",
  description: "Append a name to the probe marker file",
  inputSchema: { type: "object", properties: { name: { type: "string" } }, required: ["name"] },
};

function result(id: Request["id"], value: unknown): object {
  return { jsonrpc: "2.0", id, result: value };
}

function failure(id: Request["id"] | null, code: number, message: string): object {
  return { jsonrpc: "2.0", id, error: { code, message } };
}

function callTool(req: Request, marker: string | undefined): object {
  const params = z.looseObject({ name: z.string(), arguments: z.unknown() }).safeParse(req.params);
  if (!params.success || params.data.name !== TOUCH_TOOL.name)
    return failure(req.id, -32602, "unknown tool");
  const args = TouchArgs.safeParse(params.data.arguments);
  if (!args.success) return failure(req.id, -32602, args.error.message);
  if (marker === undefined || marker === "") {
    return result(req.id, {
      isError: true,
      content: [{ type: "text", text: "MCP_MARKER is not set" }],
    });
  }
  appendFileSync(marker, `${args.data.name}\n`);
  return result(req.id, { content: [{ type: "text", text: `touched ${args.data.name}` }] });
}

/** One response per request line; notifications (no id) get none. */
export function respond(line: string, marker: string | undefined): object | null {
  let raw: unknown;
  try {
    raw = JSON.parse(line);
  } catch {
    return failure(null, -32700, "parse error");
  }
  const parsed = Request.safeParse(raw);
  if (!parsed.success) return failure(null, -32600, "invalid request");
  const req = parsed.data;
  if (req.id === undefined) return null;
  if (req.method === "initialize") {
    const init = InitializeParams.safeParse(req.params);
    const protocolVersion = init.success ? init.data.protocolVersion : "2025-06-18";
    return result(req.id, {
      protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: "toolu-probe", version: "1.0.0" },
    });
  }
  if (req.method === "ping") return result(req.id, {});
  if (req.method === "tools/list") return result(req.id, { tools: [TOUCH_TOOL] });
  if (req.method === "tools/call") return callTool(req, marker);
  return failure(req.id, -32601, `method not found: ${req.method}`);
}

function serve(): void {
  let buffer = "";
  process.stdin.on("data", (data: Buffer) => {
    buffer += data.toString("utf8");
    for (let nl = buffer.indexOf("\n"); nl >= 0; nl = buffer.indexOf("\n")) {
      const line = buffer.slice(0, nl).trim();
      buffer = buffer.slice(nl + 1);
      const reply = line === "" ? null : respond(line, process.env.MCP_MARKER);
      if (reply !== null) process.stdout.write(`${JSON.stringify(reply)}\n`);
    }
  });
}

if (import.meta.main) serve();
