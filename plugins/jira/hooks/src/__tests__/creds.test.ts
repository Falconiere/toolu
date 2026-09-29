/**
 * Credential discovery (ported from creds.bats): reuse of an installed
 * ankitpokhrel jira-cli (config, ~/.netrc, OS keyring), env precedence, and the
 * friendly no-credentials message. The config, netrc and a `security` keyring
 * helper live in a per-test sandbox, so the host's real setup is never read.
 */
import { afterAll, afterEach, beforeEach, expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { startJira } from "./harness.ts";

const h = await startJira();
afterAll(() => h.fixture.stop());

let sandbox = "";
beforeEach(() => {
  h.fixture.plan([]);
  sandbox = mkdtempSync(join(tmpdir(), "jira-creds-"));
});
afterEach(() => rmSync(sandbox, { recursive: true, force: true }));

/** A jira-cli config in the sandbox; returns its path. */
function cliConfig(lines: Record<string, string>): string {
  const dir = join(sandbox, ".jira");
  mkdirSync(dir, { recursive: true });
  const path = join(dir, ".config.yml");
  const body = Object.entries(lines).map(([key, value]) => `${key}: ${value}\n`);
  writeFileSync(path, body.join(""));
  return path;
}

const NO_ENV_CREDS = {
  JIRA_BASE_URL: undefined,
  JIRA_PAT: undefined,
  JIRA_EMAIL: undefined,
  JIRA_API_TOKEN: undefined,
};

function basic(user: string, token: string): string {
  return `Basic ${Buffer.from(`${user}:${token}`).toString("base64")}`;
}

test("creds: reuses jira-cli server+login, token from JIRA_API_TOKEN env", async () => {
  const config = cliConfig({ server: "https://acme.atlassian.net", login: "cli@x.com", installation: "Cloud" });
  const run = await h.jira(["user", "whoami"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config, JIRA_API_TOKEN: "envtok" },
  });
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request.url).toBe("https://acme.atlassian.net/rest/api/3/myself");
  expect(request.headers["authorization"]).toBe(basic("cli@x.com", "envtok"));
});

test("creds: reuses api_token from the jira-cli config file", async () => {
  const config = cliConfig({
    server: "https://acme.atlassian.net",
    login: "cfg@x.com",
    installation: "Cloud",
    api_token: "cfgtok",
  });
  const run = await h.jira(["user", "whoami"], { env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config } });
  expect(run.status).toBe(0);
  expect(h.only().headers["authorization"]).toBe(basic("cfg@x.com", "cfgtok"));
});

test("creds: reads the API token from ~/.netrc when config/env lack it", async () => {
  const config = cliConfig({ server: "https://acme.atlassian.net", login: "nr@x.com", installation: "Cloud" });
  const netrc = join(sandbox, "netrc");
  writeFileSync(
    netrc,
    "machine other.example login a password wrong\nmachine acme.atlassian.net\n  login nr@x.com\n  password netrctok\n",
  );
  const run = await h.jira(["user", "whoami"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config, NETRC: netrc },
  });
  expect(run.status).toBe(0);
  expect(h.only().headers["authorization"]).toBe(basic("nr@x.com", "netrctok"));
});

test("creds: reads the API token from the OS keyring when env lacks it", async () => {
  const config = cliConfig({ server: "https://acme.atlassian.net", login: "kr@x.com", installation: "Cloud" });
  // A `security` on PATH standing in for the macOS keyring; it records its argv.
  const bin = join(sandbox, "bin");
  mkdirSync(bin);
  const security = join(bin, "security");
  writeFileSync(security, `#!/bin/sh\nprintf '%s\\n' "$@" > "${sandbox}/security.argv"\nprintf 'keyringtok\\n'\n`);
  chmodSync(security, 0o755);
  const run = await h.jira(["user", "whoami"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config, PATH: `${bin}:${process.env["PATH"] ?? ""}` },
  });
  expect(run.status).toBe(0);
  expect(h.only().headers["authorization"]).toBe(basic("kr@x.com", "keyringtok"));
  const argv = await Bun.file(join(sandbox, "security.argv")).text();
  expect(argv).toBe("find-generic-password\n-s\njira-cli\n-a\nkr@x.com\n-w\n");
});

test("creds: installation Cloud selects api version 3", async () => {
  const config = cliConfig({ server: "https://acme.atlassian.net", login: "c@x.com", installation: "Cloud" });
  const run = await h.jira(["project", "list"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config, JIRA_API_TOKEN: "t" },
  });
  expect(run.status).toBe(0);
  expect(h.only().url).toBe("https://acme.atlassian.net/rest/api/3/project");
});

test("creds: installation Local selects api version 2", async () => {
  const config = cliConfig({ server: "https://acme.atlassian.net", login: "c@x.com", installation: "Local" });
  const run = await h.jira(["project", "list"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: config, JIRA_PAT: "p" },
  });
  expect(run.status).toBe(0);
  expect(h.only().url).toBe("https://acme.atlassian.net/rest/api/2/project");
});

test("creds: explicit env overrides the jira-cli config", async () => {
  const config = cliConfig({ server: "https://media.example.net", login: "cli@x.com", installation: "Local" });
  const run = await h.jira(["user", "whoami"], {
    env: { JIRA_CLI_CONFIG: config, JIRA_BASE_URL: "https://acme.atlassian.net", JIRA_PAT: "envpat" },
  });
  expect(run.status).toBe(0);
  const request = h.only();
  expect(request.url).toBe("https://acme.atlassian.net/rest/api/2/myself");
  expect(request.headers["authorization"]).toBe("Bearer envpat");
});

test("creds: no credentials prints a friendly setup message (not an error) and makes no request", async () => {
  const run = await h.jira(["user", "whoami"], {
    env: { ...NO_ENV_CREDS, JIRA_CLI_CONFIG: "/nonexistent/.config.yml" },
  });
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("jira init");
  expect(run.stderr).toContain("one-time setup step");
  expect(run.stderr).toContain("Nothing is broken");
  expect(run.stdout).toBe("");
  expect(h.fixture.requests).toHaveLength(0);
});
