import { expect, test } from "bun:test";
import { appendFileSync, existsSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { openSession, PROBE_PLUGIN } from "../session.ts";

// Real sandboxes and a real loopback provider: the isolation every live probe relies on (#335).

const HostConfig = z.looseObject({
  model: z.string(),
  small_model: z.string(),
  enabled_providers: z.array(z.string()),
  provider: z.looseObject({
    probe: z.looseObject({ npm: z.string(), options: z.looseObject({ baseURL: z.string() }) }),
  }),
});

function config(sb: Sandbox): z.infer<typeof HostConfig> {
  return HostConfig.parse(JSON.parse(sb.read("opencode.json")));
}

test.concurrent("a probe session isolates the profile and enables only the scripted loopback provider", () => {
  using cache = createSandbox();
  using session = openSession(cache.root, { localPlugins: [PROBE_PLUGIN] });
  const { sb } = session;
  expect(session.env).toEqual({
    PWD: sb.project,
    HOME: sb.home,
    XDG_CONFIG_HOME: join(sb.home, ".config"),
    XDG_DATA_HOME: join(sb.home, ".local/share"),
    XDG_STATE_HOME: join(sb.home, ".local/state"),
    XDG_CACHE_HOME: join(cache.root, "xdg"),
    npm_config_cache: join(cache.root, "npm"),
    TOOLU_PROBE_LOG: join(sb.root, "probe-log.jsonl"),
    TOOLU_PROBE_CONFIG: join(sb.root, "probe-config.json"),
  });
  const host = config(sb);
  expect({
    model: host.model,
    small: host.small_model,
    enabled: host.enabled_providers,
    npm: host.provider.probe.npm,
  }).toEqual({
    model: "probe/scripted",
    small: "probe/scripted",
    enabled: ["probe"],
    npm: "@ai-sdk/openai-compatible",
  });
  expect(host.provider.probe.options.baseURL).toMatch(/^http:\/\/127\.0\.0\.1:\d+\/v1$/);
  expect(existsSync(sb.path(".opencode/plugins/probe.ts"))).toBe(true);
  expect(session.logPath.startsWith(sb.project)).toBe(false);
});

test.concurrent("probe log lines parse into typed entries", () => {
  using cache = createSandbox();
  using session = openSession(cache.root);
  appendFileSync(
    session.logPath,
    `${JSON.stringify({ kind: "after", tool: "bash", exit: 3, metadata: { exit: 3 } })}\n`,
  );
  appendFileSync(session.logPath, `${JSON.stringify({ kind: "event", type: "session.idle" })}\n`);
  expect(session.log()).toEqual([
    { kind: "after", tool: "bash", exit: 3, metadata: { exit: 3 } },
    { kind: "event", type: "session.idle" },
  ]);
});

test.concurrent("disposing a session stops its provider and removes its sandbox", async () => {
  using cache = createSandbox();
  const session = openSession(cache.root);
  const url = config(session.sb).provider.probe.options.baseURL;
  expect((await fetch(`${url}/models`)).status).toBe(404);
  const root = session.sb.root;
  session[Symbol.dispose]();
  expect(existsSync(root)).toBe(false);
  const refused = await fetch(`${url}/models`).then(
    () => null,
    (err: unknown) => err,
  );
  expect(refused).toBeInstanceOf(Error);
});
