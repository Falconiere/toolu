/**
 * Scripted OpenAI-compatible chat-completions server for the live OpenCode
 * host probes (#335). The pinned host talks to it through its bundled
 * `@ai-sdk/openai-compatible` provider; only the model's replies are scripted.
 * Plugin loading, permissions, tools, MCP and child sessions stay the host's own.
 *
 * A request without tools (title generation) gets `Probe title`. Otherwise the
 * `PROBE:<scenario>` token in the last user message, else the first, picks a script, and the
 * number of tool results since the last user message picks the step. A session
 * whose user messages carry no token (one the scenario did not prompt itself,
 * for example a nested fixer) uses the `*` script. An exhausted or unknown
 * script answers `PROBE-DONE`.
 */
import { z } from "zod";

export type ScriptStep = { tool: string; args: Record<string, unknown> };
export type Scripts = Readonly<Record<string, readonly ScriptStep[]>>;
export type RecordedRequest = { path: string; body: unknown };
type ScriptedProvider = {
  url: string;
  requests(): RecordedRequest[];
  stop(): void;
};

export const TITLE_TEXT = "Probe title";
export const DONE_TEXT = "PROBE-DONE";

const ChatRequest = z.looseObject({
  messages: z.array(z.looseObject({ role: z.string(), content: z.unknown() })),
  tools: z.array(z.unknown()).optional(),
});
type ChatMessage = z.infer<typeof ChatRequest>["messages"][number];

function chunk(delta: object, finish: string | null): object {
  const usage =
    finish === null ? {} : { usage: { prompt_tokens: 1, completion_tokens: 1, total_tokens: 2 } };
  return {
    id: "chatcmpl-probe",
    object: "chat.completion.chunk",
    created: 0,
    model: "scripted",
    choices: [{ index: 0, delta, finish_reason: finish }],
    ...usage,
  };
}

function sse(chunks: object[]): Response {
  const body = `${chunks.map((c) => `data: ${JSON.stringify(c)}\n\n`).join("")}data: [DONE]\n\n`;
  return new Response(body, { headers: { "content-type": "text/event-stream" } });
}

function textReply(text: string): Response {
  return sse([chunk({ role: "assistant", content: text }, null), chunk({}, "stop")]);
}

function toolReply(step: ScriptStep, callId: string): Response {
  const call = {
    index: 0,
    id: callId,
    type: "function",
    function: { name: step.tool, arguments: JSON.stringify(step.args) },
  };
  return sse([chunk({ role: "assistant", tool_calls: [call] }, null), chunk({}, "tool_calls")]);
}

function scenarioToken(message: ChatMessage | undefined): string | null {
  // `content` may be absent; stringify null so the token search never sees undefined.
  const text = message === undefined ? "" : JSON.stringify(message.content ?? null);
  return text.match(/PROBE:([a-z0-9.-]+)/)?.[1] ?? null;
}

/**
 * The scenario named by the last user message, else by the first (a continued
 * session's new prompt can name its own script), and how many tool results
 * follow the last user message.
 */
function scriptPosition(messages: readonly ChatMessage[]): {
  scenario: string | null;
  done: number;
} {
  const lastUser = messages.map((m) => m.role).lastIndexOf("user");
  const scenario =
    scenarioToken(messages[lastUser]) ?? scenarioToken(messages.find((m) => m.role === "user"));
  const done = messages.slice(Math.max(lastUser, 0)).filter((m) => m.role === "tool").length;
  return { scenario, done };
}

export function startScriptedProvider(scripts: Scripts): ScriptedProvider {
  const recorded: RecordedRequest[] = [];
  let calls = 0;
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    async fetch(req) {
      const path = new URL(req.url).pathname;
      const body: unknown = req.method === "POST" ? await req.json().catch(() => null) : null;
      recorded.push({ path, body });
      if (!path.endsWith("/chat/completions"))
        return Response.json({ error: "not found" }, { status: 404 });
      const parsed = ChatRequest.safeParse(body);
      if (!parsed.success) return Response.json({ error: parsed.error.message }, { status: 400 });
      if (parsed.data.tools === undefined || parsed.data.tools.length === 0)
        return textReply(TITLE_TEXT);
      const { scenario, done } = scriptPosition(parsed.data.messages);
      const step = scripts[scenario ?? "*"]?.[done];
      if (step === undefined) return textReply(DONE_TEXT);
      calls += 1;
      return toolReply(step, `call_${calls}`);
    },
  });
  return {
    url: `http://127.0.0.1:${server.port}/v1`,
    requests: () => [...recorded],
    stop: () => {
      void server.stop(true);
    },
  };
}
