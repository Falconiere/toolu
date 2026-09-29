/**
 * `permissionsAutowrite` vs bash `toolu_permissions_autowrite` (#253): twin
 * sandboxes with the same real repo, config and settings file; one runs bash,
 * the other TypeScript; the resulting files, sentinel and notice must match.
 */
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { loadConfig } from "../config-load.ts";
import { permissionsAutowrite } from "../permissions.ts";

const PERMISSIONS_SH = resolve(
  import.meta.dir,
  "../../../../../plugins/toolu/hooks/lib/permissions.sh",
);

type Scenario = {
  host?: "claude" | "codex";
  git?: boolean;
  config?: object;
  settings?: string;
  sentinel?: boolean;
};

type Outcome = { wrote: boolean; notice: string; settings: string | null; sentinel: boolean };

function arrange(scenario: Scenario): Sandbox {
  const sb = createSandbox({ git: scenario.git ?? true });
  const host = scenario.host ?? "claude";
  if (scenario.config !== undefined) sb.writeConfig(host, "project", scenario.config);
  if (scenario.settings !== undefined) sb.write(`.${host}/settings.local.json`, scenario.settings);
  if (scenario.sentinel === true) sb.write(`.${host}/tmp/.permissions-written`, "");
  return sb;
}

function envFor(sb: Sandbox, host: string): Record<string, string> {
  return { HOME: sb.home, TOOLU_PROJECT_DIR: sb.project, TOOLU_HOST_OVERRIDE: host };
}

function observe(sb: Sandbox, host: string, wrote: boolean, notice: string): Outcome {
  const file = join(sb.project, `.${host}`, "settings.local.json");
  return {
    wrote,
    // The notice names the sandbox path; compare it relative to the project.
    notice: notice.replaceAll(sb.project, "<project>"),
    settings: existsSync(file) ? readFileSync(file, "utf8") : null,
    sentinel: existsSync(join(sb.project, `.${host}`, "tmp", ".permissions-written")),
  };
}

async function viaBash(scenario: Scenario): Promise<Outcome> {
  using sb = arrange(scenario);
  const host = scenario.host ?? "claude";
  const res = await run(
    [
      "bash",
      "-c",
      '. "$1"; toolu_permissions_autowrite "$2"; echo "rc=$?"',
      "_",
      PERMISSIONS_SH,
      sb.project,
    ],
    { cwd: sb.project, env: envFor(sb, host) },
  );
  const lines = res.stdout.trimEnd().split("\n");
  const rc = lines.pop();
  return observe(sb, host, rc === "rc=0", lines.join("\n"));
}

function viaTs(scenario: Scenario): Outcome {
  using sb = arrange(scenario);
  const host = scenario.host ?? "claude";
  const env = envFor(sb, host);
  const config = loadConfig({ env, host, cwd: sb.project, warn: () => {} });
  const result = permissionsAutowrite(config, sb.project, { env });
  return observe(sb, host, result.written, result.written ? (result.notice ?? "") : "");
}

const EXISTING = JSON.stringify({
  model: "opus",
  permissions: { deny: ["Bash(rm -rf /)"], allow: ["Read", "Bash(*)", 7] },
  env: { A: "1" },
});

const SCENARIOS: Record<string, Scenario> = {
  "fresh repo writes the default set": {},
  "existing allow and deny are kept; only new rules are added": { settings: EXISTING },
  "a configured permissions.allow replaces the default": {
    config: { permissions: { allow: ["Bash(git:*)", 5, "Read"] } },
  },
  "an all-non-string permissions.allow keeps the default": {
    config: { permissions: { allow: [1] } },
  },
  "nothing new to add still rewrites the file without a notice": {
    settings: JSON.stringify({ permissions: { allow: ["Bash(*)", "Edit", "Write"] } }),
  },
  "an object allow is read through its values": {
    settings: JSON.stringify({ permissions: { allow: { a: "Edit", b: 1 } } }),
  },
  "no permissions key appends one": { settings: JSON.stringify({ theme: "dark" }) },
  "autoAllow false skips": { config: { permissions: { autoAllow: false } } },
  "a sentinel skips": { sentinel: true },
  "malformed settings stay byte-identical": { settings: '{"permissions": {"allow": [' },
  "a top-level array cannot merge and stays untouched": { settings: "[1, 2]" },
  "a string permissions value cannot merge": { settings: JSON.stringify({ permissions: "all" }) },
  "codex is a no-op": { host: "codex" },
  "outside a git repository is a no-op": { git: false },
};

for (const [label, scenario] of Object.entries(SCENARIOS)) {
  test.concurrent(`permissions: ${label}`, async () => {
    const bash = await viaBash(scenario);
    expect(viaTs(scenario)).toEqual(bash);
  });
}

test.concurrent("the default write is the documented set with a notice", () => {
  expect(viaTs({})).toEqual({
    wrote: true,
    notice:
      "toolu wrote Bash(*), Edit, Write to <project>/.claude/settings.local.json (one time only; delete a rule and it stays deleted). Add that file to .gitignore if it is not there already.",
    settings: `${JSON.stringify({ permissions: { allow: ["Bash(*)", "Edit", "Write"] } }, null, 2)}\n`,
    sentinel: true,
  });
});

test.concurrent("an invalid config envelope skips the write (fail closed)", () => {
  using sb = createSandbox({ git: true });
  mkdirSync(join(sb.project, ".claude"), { recursive: true });
  writeFileSync(join(sb.project, ".claude", "toolu.config.json"), '{"version":2}');
  const env = envFor(sb, "claude");
  const config = loadConfig({ env, host: "claude", cwd: sb.project, warn: () => {} });
  expect(permissionsAutowrite(config, sb.project, { env })).toEqual({
    written: false,
    reason: "config invalid",
  });
  expect(existsSync(join(sb.project, ".claude", "settings.local.json"))).toBe(false);
});
