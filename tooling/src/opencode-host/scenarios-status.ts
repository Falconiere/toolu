/**
 * Pinned-host status (#359): the native `statusline-status` skill reports the
 * plugins toolu actually started, every startup sends one structured
 * `toolu: status` host-log entry, the JSON protocol stream carries nothing
 * else, and no Claude Code setting reaches any OpenCode configuration.
 */
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { runHost } from "./host-run.ts";
import { SELECTION, install, selection, skills, toolResults } from "./install-host.ts";
import {
  diagnostics,
  type EntryContext,
  type EntryResult,
  type EntryScenario,
} from "./scenarios-entry.ts";
import type { ProbeSession } from "./session.ts";

const SKILL = "statusline-status";
const COMMAND = '"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"';
const RECORD = ".opencode/toolu/state/toolu/opencode-status.json";
const CONFIG_FILE = /^(?:opencode|config|tui)\.jsonc?$/u;

/** Every nonblank line of `opencode run --format json` stdout is a JSON object. */
function protocolClean(stdout: string): boolean {
  return stdout
    .split("\n")
    .filter((line) => line.trim() !== "")
    .every((line) => {
      try {
        const parsed: unknown = JSON.parse(line);
        return typeof parsed === "object" && parsed !== null;
      } catch {
        return false;
      }
    });
}

function configFiles(dir: string): string[] {
  if (!existsSync(dir)) return [];
  return readdirSync(dir)
    .filter((name) => CONFIG_FILE.test(name))
    .map((name) => join(dir, name));
}

/** No OpenCode config in the project or the isolated profile names `statusLine`, and HOME has no `.claude`. */
function noClaudeConfig(s: ProbeSession): boolean {
  const xdg = s.env.XDG_CONFIG_HOME ?? join(s.sb.home, ".config");
  const files = [
    ...configFiles(s.sb.project),
    ...configFiles(join(s.sb.project, ".opencode")),
    ...configFiles(join(xdg, "opencode")),
  ];
  return (
    files.length > 0 &&
    files.every((file) => !readFileSync(file, "utf8").includes("statusLine")) &&
    !existsSync(join(s.sb.home, ".claude"))
  );
}

/** The one structured status line in the host log. */
function statusLine(stderr: string): string {
  return stderr.split("\n").find((line) => line.includes('message="toolu: status"')) ?? "";
}

async function enabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = {
    "status.enabled": [
      { tool: "skill", args: { name: SKILL } },
      { tool: "bash", args: { command: COMMAND, description: "toolu status" } },
    ],
  };
  const files = { [SELECTION]: selection(["statusline", "jev"]) };
  using s = install(ctx, files, scripts, () => ({ permission: { bash: "allow" } }));
  const discovered = await skills(ctx, s);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:status.enabled"]);
  const results = toolResults(hostRun.events);
  const skill = results.find((result) => result.tool === "skill");
  const bash = results.find((result) => result.tool === "bash");
  const report = bash?.text ?? "";
  const logged = statusLine(hostRun.stderr);
  const observed = {
    discovered: discovered.rows.some((row) => row.name === SKILL),
    skillLoaded:
      skill?.status === "completed" &&
      skill.text.includes("TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"),
    bashRan: bash?.status === "completed",
    ready: report.includes("toolu: ready — 2 plugins (project selection)"),
    plugins: report.includes(
      "Plugins: statusline (session-start), jev (session-start, user-prompt-submit)",
    ),
    record: s.exists(RECORD) && report.includes(join(s.sb.project, RECORD)),
    statusEntries: diagnostics(hostRun.stderr, "toolu: status"),
    structured: logged.includes("statusline,jev") && logged.includes("ready"),
    protocolClean: protocolClean(hostRun.stdout),
    noClaudeConfig: noClaudeConfig(s),
  };
  const pass =
    observed.discovered &&
    observed.skillLoaded &&
    observed.bashRan &&
    observed.ready &&
    observed.plugins &&
    observed.record &&
    observed.statusEntries === 1 &&
    observed.structured &&
    observed.protocolClean &&
    observed.noClaudeConfig;
  return {
    pass,
    observed: pass ? observed : { ...observed, report: report.slice(0, 800), logged },
  };
}

async function disabled(ctx: EntryContext): Promise<EntryResult> {
  const scripts = {
    "status.disabled": [{ tool: "bash", args: { command: "true", description: "noop" } }],
  };
  const files = { [SELECTION]: selection(["jev"]) };
  using s = install(ctx, files, scripts, () => ({ permission: { bash: "allow" } }));
  const discovered = await skills(ctx, s);
  const hostRun = await runHost(ctx.bin, s, ["--print-logs", "PROBE:status.disabled"]);
  const record = s.exists(RECORD) ? readFileSync(join(s.sb.project, RECORD), "utf8") : "";
  const observed = {
    discovered: discovered.rows.some((row) => row.name === SKILL),
    statusEntries: diagnostics(hostRun.stderr, "toolu: status"),
    recordJevOnly: record.includes('"name": "jev"') && !record.includes('"name": "statusline"'),
    helper: s.exists(".opencode/toolu/state/statusline/statusline.sh"),
    noClaudeConfig: noClaudeConfig(s),
  };
  const pass =
    !observed.discovered &&
    observed.statusEntries === 1 &&
    observed.recordJevOnly &&
    !observed.helper &&
    observed.noClaudeConfig;
  return { pass, observed };
}

export const STATUS_SCENARIOS: EntryScenario[] = [
  {
    id: "status.enabled",
    claim: "The native status skill reports the started plugins; one structured log entry",
    run: enabled,
  },
  {
    id: "status.disabled",
    claim: "Without statusline selected the skill is absent and toolu still records its status",
    run: disabled,
  },
];
