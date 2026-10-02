import { expect, test } from "bun:test";
import { DONE_TEXT, TITLE_TEXT, startScriptedProvider } from "../provider.ts";

// Real loopback HTTP against the scripted provider the live OpenCode probes use (#335).

const SCRIPTS = {
  "deny.bash": [
    { tool: "bash", args: { command: "touch DENYME.txt", description: "deny" } },
    { tool: "read", args: { filePath: "a.txt" } },
  ],
};
const TOOLS = [{ type: "function", function: { name: "bash", parameters: {} } }];

async function post(url: string, body: unknown): Promise<{ status: number; text: string }> {
  const res = await fetch(`${url}/chat/completions`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
  return { status: res.status, text: await res.text() };
}

/** The `data:` JSON payloads of an SSE body, without the `[DONE]` terminator. */
function events(text: string): unknown[] {
  return text
    .split("\n\n")
    .map((frame) => frame.replace(/^data: /, ""))
    .filter((data) => data !== "" && data !== "[DONE]")
    .map((data): unknown => JSON.parse(data));
}

test.concurrent("scripted step streams the tool call named by the scenario token", async () => {
  const provider = startScriptedProvider(SCRIPTS);
  try {
    const res = await post(provider.url, {
      messages: [{ role: "user", content: "PROBE:deny.bash" }],
      tools: TOOLS,
    });
    expect(res.status).toBe(200);
    expect(events(res.text)).toMatchObject([
      {
        choices: [
          {
            delta: {
              tool_calls: [
                {
                  id: "call_1",
                  function: {
                    name: "bash",
                    arguments: '{"command":"touch DENYME.txt","description":"deny"}',
                  },
                },
              ],
            },
          },
        ],
      },
      { choices: [{ finish_reason: "tool_calls" }] },
    ]);
    expect(provider.requests()).toHaveLength(1);
  } finally {
    provider.stop();
  }
});

test.concurrent("tool results since the last user message advance the script, then it finishes", async () => {
  const provider = startScriptedProvider(SCRIPTS);
  try {
    const user = { role: "user", content: [{ type: "text", text: "PROBE:deny.bash" }] };
    const tool = { role: "tool", content: "toolu-probe: denied before execution" };
    const second = await post(provider.url, {
      messages: [user, { role: "assistant", content: "" }, tool],
      tools: TOOLS,
    });
    expect(events(second.text)).toMatchObject([
      { choices: [{ delta: { tool_calls: [{ function: { name: "read" } }] } }] },
      {},
    ]);
    const done = await post(provider.url, { messages: [user, tool, tool], tools: TOOLS });
    expect(events(done.text)).toMatchObject([
      { choices: [{ delta: { content: DONE_TEXT } }] },
      { choices: [{ finish_reason: "stop" }] },
    ]);
  } finally {
    provider.stop();
  }
});

test.concurrent("requests without tools get the title text; unknown scenarios finish", async () => {
  const provider = startScriptedProvider(SCRIPTS);
  try {
    const title = await post(provider.url, {
      messages: [{ role: "user", content: "PROBE:deny.bash" }],
    });
    expect(events(title.text)).toMatchObject([
      { choices: [{ delta: { content: TITLE_TEXT } }] },
      {},
    ]);
    const unknown = await post(provider.url, {
      messages: [{ role: "user", content: "PROBE:no.such" }],
      tools: TOOLS,
    });
    expect(events(unknown.text)).toMatchObject([
      { choices: [{ delta: { content: DONE_TEXT } }] },
      {},
    ]);
  } finally {
    provider.stop();
  }
});

test.concurrent("malformed bodies are rejected with 400 and other paths with 404", async () => {
  const provider = startScriptedProvider(SCRIPTS);
  try {
    expect((await post(provider.url, { messages: "nope" })).status).toBe(400);
    const missing = await fetch(`${provider.url}/models`);
    expect(missing.status).toBe(404);
    expect(provider.requests().map((r) => r.path)).toEqual(["/v1/chat/completions", "/v1/models"]);
  } finally {
    provider.stop();
  }
});
