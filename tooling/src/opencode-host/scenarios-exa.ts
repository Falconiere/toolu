/** Pinned-host exa-search discovery, HTTPS transport, credentials and lifecycle (#349). */
import { existsSync, readFileSync, readlinkSync } from "node:fs";
import { join } from "node:path";
import { startHttpsFixture, type HttpsFixture } from "@toolu/conformance/https-fixture";
import { z } from "zod";
import { runHost } from "./host-run.ts";
import { SELECTION, install, selection, skills, toolResults } from "./install-host.ts";
import {
  diagnostics,
  ROOT,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import { messagesText } from "./scenario.ts";
import type { ProbeSession } from "./session.ts";

const SKILL = "exa-search-exa-search";
const HELPER = ".opencode/toolu/state/exa-search/search.sh";
const KEY = "exa-fixture-sentinel-key";
const COMMAND = '"$TOOLU_CONFIG_DIR/exa-search/search.sh"';

function files(): Record<string, string> {
  return { [SELECTION]: selection(["exa-search"]) };
}

function helperPublished(s: ProbeSession): boolean {
  const helper = join(s.sb.project, HELPER);
  if (!existsSync(helper)) return false;
  const source = readlinkSync(helper);
  return (
    source.endsWith("/plugins/exa-search/hooks/dist/search.js") &&
    readFileSync(source).equals(readFileSync(join(ROOT, "plugins/exa-search/hooks/dist/search.js")))
  );
}

function projectText(s: ProbeSession, file: string): string {
  return s.exists(file) ? readFileSync(join(s.sb.project, file), "utf8") : "";
}

function shellQuote(value: string): string {
  return `'${value.replaceAll("'", "'\\''")}'`;
}

async function enabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = { "exa.enabled": [{ tool: "skill", args: { name: SKILL } }] };
  using s = install(ctx, files(), scripts, () => ({ permission: { bash: "allow" } }));
  s.env.EXA_API_KEY = KEY;
  const discovered = await skills(ctx, s);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:exa.enabled"]);
  const loaded = toolResults(hostRun.events).find((result) => result.tool === "skill");
  const skillText = loaded?.text ?? "";
  const system = messagesText(s, "system");
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    discovered: discovered.rows.some((row) => row.name === SKILL),
    loaded: loaded?.status === "completed",
    openCodePath: skillText.includes("# OpenCode") && skillText.includes("exa-search/search.sh"),
    commands:
      skillText.includes("search -q") &&
      skillText.includes("crawl <url>") &&
      skillText.includes("similar <url>"),
    helper: helperPublished(s),
    context: system.includes(join(s.sb.project, HELPER)) && system.includes(SKILL),
    keyHidden: !JSON.stringify(s.requests()).includes(KEY) && !skillText.includes(KEY),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

function transportCommand(env: Readonly<Record<string, string>>): string {
  const proxy = Object.entries(env)
    .map(([name, value]) => `${name}=${shellQuote(value)}`)
    .join(" ");
  const cli = `env ${proxy} ${COMMAND}`;
  const cases = [
    ["search", "search -q needle"],
    ["crawl", "crawl https://example.test/page"],
    ["similar", "similar https://example.test/page"],
    ["http", "search -q denied"],
    ["json", "search -q malformed"],
    ["connection", "search -q disconnected"],
  ];
  return cases
    .flatMap(([name, args]) => [
      `${cli} ${args} > ${name}.out 2> ${name}.err`,
      `printf '${name}=%s\\n' "$?" >> exits.txt`,
    ])
    .join("\n");
}

function transportResult(
  s: ProbeSession,
  hostRun: Awaited<ReturnType<typeof runHost>>,
  fixture: HttpsFixture,
): EntryResult {
  const bash = toolResults(hostRun.events).find((result) => result.tool === "bash");
  const requests = fixture.requests;
  const bodies = requests.map((request) =>
    z.record(z.string(), z.unknown()).parse(JSON.parse(request.body)),
  );
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    ran: bash?.status === "completed" && helperPublished(s),
    paths:
      requests.map((request) => request.path).join(",") ===
      "/search,/contents,/findSimilar,/search,/search",
    methods: requests.every((request) => request.method === "POST"),
    bodies:
      bodies[0]?.["query"] === "needle" &&
      Array.isArray(bodies[1]?.["urls"]) &&
      bodies[2]?.["url"] === "https://example.test/page",
    keySent: requests.every((request) => request.headers["x-api-key"] === KEY),
    success:
      projectText(s, "search.out").includes('"kind": "search"') &&
      projectText(s, "crawl.out").includes('"kind": "crawl"') &&
      projectText(s, "similar.out").includes('"kind": "similar"'),
    failures:
      projectText(s, "exits.txt") ===
        "search=0\ncrawl=0\nsimilar=0\nhttp=22\njson=5\nconnection=1\n" &&
      projectText(s, "http.err").includes("HTTP 401") &&
      projectText(s, "json.err").includes("response is not JSON") &&
      projectText(s, "connection.err").includes("request failed"),
    secretHidden:
      !JSON.stringify(s.requests()).includes(KEY) &&
      !JSON.stringify(hostRun.events).includes(KEY) &&
      !projectText(s, "http.err").includes(KEY),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function transport(ctx: EntryContext): Promise<EntryResult> {
  const fixture = await startHttpsFixture(["api.exa.ai"]);
  try {
    fixture.plan([
      { body: '{"kind":"search"}' },
      { body: '{"kind":"crawl"}' },
      { body: '{"kind":"similar"}' },
      { status: 401, body: '{"error":"denied"}' },
      { body: "<html>", contentType: "text/html" },
      { close: true },
    ]);
    const scripts = {
      "exa.transport": [
        {
          tool: "bash",
          args: { command: transportCommand(fixture.env), description: "Exa HTTPS fixture" },
        },
      ],
    };
    using s = install(ctx, files(), scripts, () => ({ permission: { bash: "allow" } }));
    s.env.EXA_API_KEY = KEY;
    const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:exa.transport"]);
    return transportResult(s, hostRun, fixture);
  } finally {
    await fixture.stop();
  }
}

async function noKey(ctx: EntryContext): Promise<EntryResult> {
  const scripts = {
    "exa.no-key": [
      {
        tool: "bash",
        args: {
          command: `${COMMAND} search -q needle > no-key.out 2> no-key.err; printf 'exit=%s\\n' "$?"`,
          description: "Exa without key",
        },
      },
    ],
  };
  using s = install(ctx, files(), scripts, () => ({ permission: { bash: "allow" } }));
  s.env.EXA_API_KEY = "";
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:exa.no-key"]);
  const bash = toolResults(hostRun.events).find((result) => result.tool === "bash");
  const system = messagesText(s, "system");
  const observed = {
    ready: diagnostics(hostRun.stderr, "toolu: ready") === 1,
    helper: helperPublished(s),
    called: bash?.status === "completed" && bash.text.includes("exit=1"),
    diagnostic: projectText(s, "no-key.err").includes("EXA_API_KEY unset"),
    fallback: system.includes("websearch") && system.includes("webfetch"),
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

async function disabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = {
    "exa.disabled": [
      { tool: "bash", args: { command: "printf 'core-ready'", description: "ready" } },
    ],
  };
  using s = install(ctx, files(), scripts, () => ({ permission: { bash: "allow" } }));
  const first = await skills(ctx, s);
  const firstHelper = helperPublished(s);
  s.sb.write(SELECTION, selection(["toolu"]));
  const second = await skills(ctx, s);
  const previousRequests = s.requests().length;
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:exa.disabled"]);
  const newRequests = s.requests().slice(previousRequests);
  const bash = toolResults(hostRun.events).find((result) => result.tool === "bash");
  const observed = {
    firstSkill: first.rows.some((row) => row.name === SKILL),
    firstHelper,
    skillRemoved: !second.rows.some((row) => row.name === SKILL),
    helperRemoved: !existsSync(join(s.sb.project, HELPER)),
    contextRemoved: !JSON.stringify(newRequests).includes("exa-search helper:"),
    coreReady: diagnostics(hostRun.stderr, "toolu: ready") === 1 && bash?.status === "completed",
  };
  return { pass: Object.values(observed).every(Boolean), observed };
}

export const EXA_SCENARIOS: EntryScenario[] = [
  { id: "exa.enabled", claim: "Native exa skill and helper path reach OpenCode", run: enabled },
  { id: "exa.transport", claim: "Published exa helper preserves HTTPS behavior", run: transport },
  { id: "exa.no-key", claim: "No key yields an explicit useful fallback", run: noKey },
  { id: "exa.disabled", claim: "Disabling exa removes owned contributions", run: disabled },
];
