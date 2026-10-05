/**
 * Runs the committed jira bundle by path, as the published symlink runs it,
 * against the loopback HTTPS fixture posing as acme.atlassian.net (and a media
 * host for attachment redirects). Responses are recorded real Jira bodies from
 * fixtures/. The environment never inherits a developer's Jira or host setup.
 */
import { bundlePath, pluginRoot, publishedArgv } from "@toolu/conformance/harness/entry-command";
import { type HttpsFixture, startHttpsFixture } from "@toolu/conformance/https-fixture";
import { readFileSync } from "node:fs";
import { join } from "node:path";

export const BUNDLE = bundlePath(pluginRoot("jira"), "jira");
/** The jira CLI as its published symlink runs it, or its selected Rust command. */
export const JIRA_ARGV = publishedArgv("jira", "jira");
export const FIXTURES = join(import.meta.dir, "fixtures");
export const BASE = "https://acme.atlassian.net";

export function fixtureBody(name: string): string {
  return readFileSync(name.startsWith("/") ? name : join(FIXTURES, name), "utf8");
}

export type EnvPatch = Readonly<Record<string, string | undefined>>;

export interface JiraRun {
  readonly status: number;
  readonly stdout: string;
  readonly stderr: string;
}

/** Variables that would let the host's real Jira or host setup leak into a run. */
const SCRUBBED = /^(_?JIRA_|TOOLU_|PLUGIN_ROOT$|CODEX_HOME$|CLAUDE_CONFIG_DIR$)/;

export function jiraEnv(fixture: HttpsFixture, patch: EnvPatch = {}): Record<string, string> {
  const env: Record<string, string | undefined> = {
    ...process.env,
    ...fixture.env,
  };
  for (const name of Object.keys(env)) if (SCRUBBED.test(name)) delete env[name];
  Object.assign(env, {
    JIRA_BASE_URL: BASE,
    JIRA_PAT: "tok",
    // Credential discovery points at nothing: never the host's jira-cli or netrc.
    JIRA_CLI_CONFIG: "/dev/null",
    NETRC: "/dev/null",
    ...patch,
  });
  const clean: Record<string, string> = {};
  for (const [name, value] of Object.entries(env)) if (value !== undefined) clean[name] = value;
  return clean;
}

export interface JiraHarness {
  readonly fixture: HttpsFixture;
  jira(args: readonly string[], options?: { env?: EnvPatch; cwd?: string }): Promise<JiraRun>;
  /** Queues recorded fixture bodies, one per request; the last repeats. */
  respond(...names: readonly string[]): void;
  /** The one recorded request, as `https://<host><path>` plus its parts. */
  only(): { url: string; method: string; headers: Record<string, string>; body: string };
  /** Every recorded request's `https://<host><path>`. */
  urls(): string[];
}

export async function startJira(): Promise<JiraHarness> {
  const fixture = await startHttpsFixture(["acme.atlassian.net", "media.example.net"]);
  const urlOf = (request: { path: string; headers: Readonly<Record<string, string>> }) =>
    `https://${request.headers["host"] ?? ""}${request.path}`;
  return {
    fixture,
    async jira(args, options = {}) {
      const child = Bun.spawn([...JIRA_ARGV, ...args], {
        env: jiraEnv(fixture, options.env),
        cwd: options.cwd ?? import.meta.dir,
        stdout: "pipe",
        stderr: "pipe",
      });
      const [stdout, stderr, status] = await Promise.all([
        new Response(child.stdout).text(),
        new Response(child.stderr).text(),
        child.exited,
      ]);
      return { status, stdout, stderr };
    },
    respond(...names) {
      fixture.plan(names.map((name) => ({ body: fixtureBody(name) })));
    },
    only() {
      if (fixture.requests.length !== 1) {
        throw new Error(`expected one request, got ${fixture.requests.length}`);
      }
      const [request] = fixture.requests;
      if (request === undefined) throw new Error("no request recorded");
      return { ...request, headers: { ...request.headers }, url: urlOf(request) };
    },
    urls() {
      return fixture.requests.map(urlOf);
    },
  };
}
