/**
 * Loader scenarios (#335): how the pinned host discovers, invokes and fails
 * plugin modules — local files, config entries, default `PluginModule`s, a
 * throwing init and an exported helper function.
 */
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { runHost, toolStates } from "./host-run.ts";
import {
  entries,
  precondition,
  toolRequestCount,
  verdict,
  type Observation,
  type Scenario,
  type ScenarioContext,
} from "./scenario.ts";
import { DENY_MARKER, DENY_MESSAGE, openSession, PROBE_PLUGIN, PROBES_DIR } from "./session.ts";

const DOCUMENTED_INPUT = ["$", "client", "directory", "project", "worktree"];

async function localFile(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, { localPlugins: [PROBE_PLUGIN] });
  await runHost(ctx.bin, session, ["PROBE:load.local-file"]);
  const loads = entries(session, "load");
  precondition(
    "load.local-file",
    toolRequestCount(session) > 0,
    "the session never reached the model",
  );
  const keys = loads[0]?.inputKeys;
  const inputKeys = Array.isArray(keys) ? keys.join(",") : "";
  const observed = {
    loaded: loads.length === 1,
    optionsDelivered: loads[0]?.options !== null,
    inputKeys,
  };
  return verdict(
    loads.length === 1 && DOCUMENTED_INPUT.every((key) => inputKeys.split(",").includes(key)),
    observed,
  );
}

async function configFile(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    probeConfig: () => ({ denyMarker: DENY_MARKER }),
    config: () => ({
      plugin: [[pathToFileURL(PROBE_PLUGIN).href, { probe: "config" }]],
      permission: { bash: "allow" },
    }),
    scripts: {
      "load.config-file": [
        {
          tool: "bash",
          args: { command: `touch ${DENY_MARKER}-config.txt`, description: "probe" },
        },
      ],
    },
  });
  const run = await runHost(ctx.bin, session, ["PROBE:load.config-file"]);
  const loads = entries(session, "load");
  const options = loads[0]?.options;
  const optionsDelivered =
    typeof options === "object" && options !== null && Reflect.get(options, "probe") === "config";
  const denied = toolStates(run).some((t) => t.tool === "bash" && t.error === DENY_MESSAGE);
  const hooksActive = denied && !session.exists(`${DENY_MARKER}-config.txt`);
  return verdict(loads.length === 1 && optionsDelivered && hooksActive, {
    loaded: loads.length === 1,
    optionsDelivered,
    hooksActive,
  });
}

async function moduleDefault(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [join(PROBES_DIR, "module-default.ts")],
  });
  await runHost(ctx.bin, session, ["PROBE:load.module-default"]);
  precondition(
    "load.module-default",
    toolRequestCount(session) > 0,
    "the session never reached the model",
  );
  const serverInvoked = entries(session, "module-server-called").length === 1;
  const namedHelperInvoked = entries(session, "named-helper-called").length > 0;
  return verdict(serverInvoked && !namedHelperInvoked, { serverInvoked, namedHelperInvoked });
}

async function initThrow(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [join(PROBES_DIR, "init-throw.ts")],
    config: () => ({ permission: { bash: "allow" } }),
    scripts: {
      "load.init-throw": [
        { tool: "bash", args: { command: "touch init-throw.txt", description: "probe" } },
      ],
    },
  });
  const run = await runHost(ctx.bin, session, ["--print-logs", "PROBE:load.init-throw"]);
  precondition(
    "load.init-throw",
    toolRequestCount(session) > 0,
    "the session never reached the model",
  );
  const loadFailureLogged = run.stderr.includes("failed to load plugin");
  const toolRanUnguarded = session.exists("init-throw.txt");
  return verdict(!toolRanUnguarded, { loadFailureLogged, toolRanUnguarded });
}

async function helperExport(ctx: ScenarioContext): Promise<Observation> {
  using session = openSession(ctx.cacheRoot, {
    localPlugins: [join(PROBES_DIR, "helper-export.ts")],
  });
  const run = await runHost(ctx.bin, session, ["PROBE:load.helper-export"]);
  precondition(
    "load.helper-export",
    entries(session, "plugin-called").length === 1,
    "the plugin export never ran",
  );
  const helperInvokedAsPlugin = entries(session, "helper-called").length > 0;
  const promptFailed = run.exitCode !== 0 || run.events.some((event) => event.type === "error");
  return verdict(!helperInvokedAsPlugin, { helperInvokedAsPlugin, promptFailed });
}

export const LOAD_SCENARIOS: Scenario[] = [
  {
    id: "load.local-file",
    axis: "load",
    kind: "loader",
    mechanism: ".opencode/plugins/*.ts",
    claim:
      "A typed plugin file in .opencode/plugins/ is loaded and called with the documented PluginInput",
    run: localFile,
  },
  {
    id: "load.config-file",
    axis: "load",
    kind: "config",
    mechanism: 'opencode.json plugin: [["file://…", options]]',
    claim: "A plugin listed in opencode.json is loaded with its options and its hooks are active",
    run: configFile,
  },
  {
    id: "load.module-default",
    axis: "load",
    kind: "loader",
    mechanism: "export default { id, server } (PluginModule)",
    claim: "A default PluginModule export is loaded through its server function only",
    run: moduleDefault,
  },
  {
    id: "load.init-throw",
    axis: "load",
    kind: "loader",
    mechanism: "plugin function throws during init",
    claim: "The host refuses to run tools when a plugin fails to initialize (fail closed)",
    run: initThrow,
  },
  {
    id: "load.helper-export",
    axis: "load",
    kind: "loader",
    mechanism: "module exports a plugin and a helper function",
    claim: "The loader ignores exported functions that are not plugins",
    run: helperExport,
  },
];
