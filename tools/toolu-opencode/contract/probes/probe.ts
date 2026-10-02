/**
 * Typed host-contract probe (#335). A documented `Plugin` — a function that
 * returns `Hooks` — whose behavior the live harness selects through the JSON
 * file named by `TOOLU_PROBE_CONFIG`. Every hook appends one JSON line to
 * `TOOLU_PROBE_LOG`, so the harness reads what the pinned host actually
 * invoked. Dependency-free at runtime: the SDK import is type-only.
 *
 * Deliberately strict: hooks do not guard the outputs the SDK declares, and
 * logging and config errors are not swallowed. A probe must surface a host
 * change or a broken harness — the harness preconditions turn a missing log
 * entry into a hard failure — never hide it. Logging is synchronous so the log
 * order matches hook order (`permission.order` compares positions).
 */
import type { Hooks, Plugin, PluginInput } from "@opencode-ai/plugin";
import { appendFileSync, existsSync, readFileSync } from "node:fs";

type ProbeConfig = {
  denyMarker: string | undefined;
  afterAppend: string | undefined;
  permissionAsk: "allow" | "deny" | undefined;
  chatPart: string | undefined;
  system: string | undefined;
  compaction: string | undefined;
  shellEnv: string | undefined;
  preAdvisory: string | undefined;
  configSkillsPath: string | undefined;
  toast: string | undefined;
};

const DENY_MESSAGE = "toolu-probe: denied before execution";

function record(entry: Record<string, unknown>): void {
  const path = process.env.TOOLU_PROBE_LOG;
  if (path !== undefined && path !== "") appendFileSync(path, `${JSON.stringify(entry)}\n`);
}

function str(raw: Record<string, unknown>, key: string): string | undefined {
  const value = raw[key];
  return typeof value === "string" ? value : undefined;
}

function readConfig(): ProbeConfig {
  const path = process.env.TOOLU_PROBE_CONFIG;
  const parsed: unknown =
    path === undefined || path === "" || !existsSync(path)
      ? {}
      : JSON.parse(readFileSync(path, "utf8"));
  const raw: Record<string, unknown> =
    typeof parsed === "object" && parsed !== null ? Object.fromEntries(Object.entries(parsed)) : {};
  const ask = str(raw, "permissionAsk");
  return {
    denyMarker: str(raw, "denyMarker"),
    afterAppend: str(raw, "afterAppend"),
    permissionAsk: ask === "allow" || ask === "deny" ? ask : undefined,
    chatPart: str(raw, "chatPart"),
    system: str(raw, "system"),
    compaction: str(raw, "compaction"),
    shellEnv: str(raw, "shellEnv"),
    preAdvisory: str(raw, "preAdvisory"),
    configSkillsPath: str(raw, "configSkillsPath"),
    toast: str(raw, "toast"),
  };
}

function toolHooks(cfg: ProbeConfig): Hooks {
  return {
    "tool.execute.before": async (input, output) => {
      const args: unknown = output.args;
      record({ kind: "before", tool: input.tool, sessionID: input.sessionID, args });
      if (cfg.preAdvisory !== undefined)
        Object.assign(output, { additionalContext: cfg.preAdvisory });
      if (cfg.denyMarker !== undefined && JSON.stringify(args).includes(cfg.denyMarker)) {
        throw new Error(DENY_MESSAGE);
      }
    },
    "tool.execute.after": async (input, output) => {
      const metadata: unknown = output.metadata;
      const exit =
        typeof metadata === "object" && metadata !== null
          ? Reflect.get(metadata, "exit")
          : undefined;
      record({
        kind: "after",
        tool: input.tool,
        sessionID: input.sessionID,
        keys: Object.keys(output).sort(),
        exit,
        metadata,
      });
      if (cfg.afterAppend !== undefined && typeof output.output === "string") {
        output.output = `${output.output}\n${cfg.afterAppend}`;
      }
    },
    "permission.ask": async (input, output) => {
      record({ kind: "permission.ask", permission: input.type, status: output.status });
      if (cfg.permissionAsk !== undefined) output.status = cfg.permissionAsk;
    },
    "shell.env": async (_input, output) => {
      if (cfg.shellEnv !== undefined) output.env.TOOLU_PROBE_ENV = cfg.shellEnv;
    },
    "command.execute.before": async (input) => {
      record({ kind: "command.before", command: input.command, arguments: input.arguments });
    },
  };
}

function contextHooks(cfg: ProbeConfig): Hooks {
  return {
    "chat.message": async (input, output) => {
      record({ kind: "chat.message", sessionID: input.sessionID, agent: input.agent ?? null });
      if (cfg.chatPart === undefined) return;
      const { id: messageID, sessionID } = output.message;
      output.parts.push({
        id: `prt_probe${Date.now()}`,
        sessionID,
        messageID,
        type: "text",
        text: cfg.chatPart,
        synthetic: true,
      });
    },
    "experimental.chat.system.transform": async (_input, output) => {
      if (cfg.system !== undefined) output.system.push(cfg.system);
    },
    "experimental.session.compacting": async (input, output) => {
      record({ kind: "compacting", sessionID: input.sessionID });
      if (cfg.compaction !== undefined) output.context.push(cfg.compaction);
    },
  };
}

function configHook(cfg: ProbeConfig): Hooks {
  return {
    config: async (config) => {
      if (cfg.configSkillsPath === undefined) return;
      config.command = {
        ...config.command,
        "toolu-probe-cmd": {
          template: "probe command",
          description: "Injected by the config hook",
        },
      };
      config.agent = {
        ...config.agent,
        "toolu-probe-agent": {
          description: "Injected by the config hook",
          mode: "subagent",
          prompt: "probe",
        },
      };
      // `skills` is accepted by the pinned host but absent from the SDK's `Config` type.
      Reflect.set(config, "skills", { paths: [cfg.configSkillsPath] });
      config.instructions = [...(config.instructions ?? []), `${cfg.configSkillsPath}/README.md`];
    },
  };
}

function eventHook(cfg: ProbeConfig, client: PluginInput["client"]): Hooks {
  return {
    event: async ({ event }) => {
      record({ kind: "event", type: event.type });
      if (event.type !== "session.created" || cfg.toast === undefined) return;
      // A rejected toast is an observation for ui.toast, not a harness failure.
      const shown = await client.tui
        .showToast({ body: { message: cfg.toast, variant: "info" } })
        .then((res) => res.data === true)
        .catch((err: unknown) => (err instanceof Error ? err.message : String(err)));
      record({
        kind: "toast",
        ok: shown === true,
        ...(shown === true || shown === false ? {} : { error: shown }),
      });
    },
  };
}

export const TooluContractProbe: Plugin = async (input, options) => {
  const cfg = readConfig();
  record({ kind: "load", inputKeys: Object.keys(input).sort(), options: options ?? null });
  return {
    ...toolHooks(cfg),
    ...contextHooks(cfg),
    ...configHook(cfg),
    ...eventHook(cfg, input.client),
  };
};
