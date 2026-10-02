import { afterEach, expect, test } from "bun:test";
import { createOpencodeClient } from "@opencode-ai/sdk";
import { bindHostContext } from "../context.ts";

type Recorded = { path: string; body: unknown };
const servers: Array<{ stop: (force?: boolean) => unknown }> = [];

afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true);
});

type HostApi = { url: string; recorded: Recorded[]; received: Promise<void> };

/** A loopback stand-in for the host's HTTP API: records each request, then replies. */
function hostApi(reply: () => Promise<Response>): HostApi {
  const recorded: Recorded[] = [];
  const { promise: received, resolve } = Promise.withResolvers<void>();
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch: async (request) => {
      recorded.push({ path: new URL(request.url).pathname, body: await request.json() });
      resolve();
      return reply();
    },
  });
  servers.push(server);
  return { url: server.url.href, recorded, received };
}

function client(baseUrl: string): ReturnType<typeof createOpencodeClient> {
  return createOpencodeClient({ baseUrl });
}

test("binds directory, project root, options and a defined-only env", () => {
  const binding = bindHostContext(
    { client: client("http://127.0.0.1:9"), directory: "/repo/pkg", worktree: "/repo" },
    { repoRoot: "/opt/toolu" },
    { KEEP: "1", DROP: undefined },
  );
  expect(binding.directory).toBe("/repo/pkg");
  expect(binding.projectRoot).toBe("/repo");
  expect(binding.repoRootOption).toBe("/opt/toolu");
  expect(binding.optionsError).toBeUndefined();
  expect(binding.env).toEqual({ KEEP: "1" });
});

test("the diagnostic reaches the host log endpoint as service toolu", async () => {
  const api = hostApi(() => Promise.resolve(Response.json(true)));
  const binding = bindHostContext(
    { client: client(api.url), directory: "/w", worktree: "/w" },
    undefined,
    {},
  );
  await binding.log("error", "toolu: not ready: probe");
  expect(api.recorded).toEqual([
    {
      path: "/log",
      body: { service: "toolu", level: "error", message: "toolu: not ready: probe" },
    },
  ]);
});

test("a host that never answers cannot hold the diagnostic past its timeout", async () => {
  const api = hostApi(() => new Promise<Response>(() => undefined));
  const binding = bindHostContext(
    { client: client(api.url), directory: "/w", worktree: "/w" },
    undefined,
    {},
    50,
  );
  const started = performance.now();
  await binding.log("info", "toolu: ready (1 plugins, 0 startup artifacts)");
  expect(performance.now() - started).toBeLessThan(2_000);
  // The request may land after the 50 ms bound on a loaded runner; the host still answers nothing.
  await api.received;
  expect(api.recorded).toHaveLength(1);
});

test("an unreachable host log resolves without throwing", async () => {
  const api = hostApi(() => Promise.resolve(Response.json(true)));
  servers.splice(0).forEach((server) => server.stop(true));
  const binding = bindHostContext(
    { client: client(api.url), directory: "/w", worktree: "/w" },
    undefined,
    {},
  );
  expect(await binding.log("error", "unreachable")).toBeUndefined();
});
