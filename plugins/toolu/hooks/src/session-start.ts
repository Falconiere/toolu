/**
 * SessionStart (#263): toolu's Session Protocol, model routing, one-time
 * notices, tool mandates and dependency warnings as `additionalContext`, with
 * byte-identical text to the bash `session-start.sh` it replaces. The
 * `systemMessage` is the event title followed, on startup and resume, by the
 * runtime line #250 added: which Bun runs toolu's hooks. Context-only: any
 * failure exits 0.
 */
import { join } from "node:path";
import { enabled, loadConfig, permissionsAutowrite, type LoadedConfig } from "@toolu/core/config";
import { configRoot, detectHost, type HostName } from "@toolu/core/host";
import { runtimeDiagnostic } from "@toolu/core/launcher";
import { renderHookOutput } from "@toolu/core/startup";
import { sweepState } from "@toolu/core/state";
import {
  gitToplevel,
  jqAlt,
  member,
  parseStdin,
  stripTrailingNewlines,
} from "./lifecycle/bash-compat.ts";
import { dependencyWarning } from "./lifecycle/dependency-warning.ts";
import { projectFacts } from "./lifecycle/project-detect.ts";
import { docParts, eventTitle } from "./lifecycle/session-docs.ts";
import { housekeeping } from "./lifecycle/session-housekeeping.ts";
import { deliveryFlowNotice, gatePresetNotice } from "./lifecycle/session-notices.ts";
import { mandateBlock, missingToolsWarning } from "./lifecycle/tool-mandates.ts";

type Env = Record<string, string | undefined>;

const DOCS = join(import.meta.dir, "..", "docs");

/** `.source // .session_event // .event // "startup"`; unparseable input is startup. */
function sessionEvent(input: string): string {
  const doc = parseStdin(input);
  const picked = ["source", "session_event", "event"]
    .map((key) => member(doc, key))
    .find((value) => value !== undefined && value !== null && value !== false);
  const event = jqAlt(picked, "startup");
  return event === "" || event === "null" ? "startup" : event;
}

/** `git rev-parse --abbrev-ref HEAD`; an unborn branch prints `HEAD`. */
function branchLine(cwd: string, env: Env): string | undefined {
  if (Bun.which("git", { PATH: env.PATH ?? "" }) === null) return undefined;
  const res = Bun.spawnSync(["git", "rev-parse", "--abbrev-ref", "HEAD"], {
    cwd,
    env,
    stdout: "pipe",
    stderr: "ignore",
  });
  const branch = stripTrailingNewlines(res.stdout.toString());
  return branch === "" ? undefined : `Branch: ${branch}`;
}

/** `toolu_plugin_root`: Codex's PLUGIN_ROOT, else CLAUDE_PLUGIN_ROOT. */
function pluginRootOf(env: Env, host: HostName): string {
  const codex = host === "codex" ? (env.PLUGIN_ROOT ?? "") : "";
  return codex !== "" ? codex : (env.CLAUDE_PLUGIN_ROOT ?? "");
}

type Session = { event: string; env: Env; host: HostName; root: string; config: LoadedConfig };

function contextParts({ event, env, host, root, config }: Session): string[] {
  const cwd = process.cwd();
  const top = gitToplevel(cwd, env);
  const project = top === "" ? cwd : top;
  const verbose = (env.TOOLU_VERBOSE ?? "") !== "" && env.TOOLU_VERBOSE !== "0";
  const facts = projectFacts(top, env, verbose);
  const parts = docParts({ docs: DOCS, event, config, host, facts, verbose });
  if (facts.name !== "") parts.push(`Project: ${facts.name}`);
  sweepState(project, { env, host, config });
  const quiet = { ...config, warn: () => undefined };
  const permissions = permissionsAutowrite(quiet, project, { env, cwd });
  const mandates = { config, env, host };
  const extras: (string | null | undefined)[] = [
    permissions.written ? permissions.notice : undefined,
    gatePresetNotice(root, config.data),
    deliveryFlowNotice(root, host),
    missingToolsWarning(mandates),
    mandateBlock(mandates),
    dependencyWarning({ env, host, pluginRoot: pluginRootOf(env, host), projectRoot: project }),
    branchLine(cwd, env),
  ];
  for (const part of extras)
    if (part !== undefined && part !== null && part !== "") parts.push(part);
  return parts;
}

function systemMessage(event: string): string {
  const title = eventTitle(event);
  if (event !== "startup" && event !== "resume") return title;
  const runtime = runtimeDiagnostic(process.execPath, Bun.version).systemMessage;
  return title === "" ? runtime : `${title}\n${runtime}`;
}

async function main(): Promise<void> {
  const env = process.env;
  const host = detectHost({ env });
  const root = configRoot({ env, host });
  housekeeping(env, host, root);
  const event = sessionEvent(await Bun.stdin.text());
  const config = loadConfig({ env, host });
  if (!enabled(config, "hooks", "session-start")) {
    if (event === "startup" || event === "resume") {
      process.stdout.write(renderHookOutput({ systemMessage: systemMessage(event) }, true));
    }
    return;
  }
  const additionalContext = contextParts({ event, env, host, root, config }).join("\n\n");
  process.stdout.write(
    renderHookOutput(
      {
        hookSpecificOutput: { hookEventName: "SessionStart", additionalContext },
        systemMessage: systemMessage(event),
      },
      true,
    ),
  );
}

try {
  await main();
} catch (error) {
  process.stderr.write(`toolu session-start: ${String(error)}\n`);
}
