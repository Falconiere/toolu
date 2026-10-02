/**
 * OpenCode plugin entry (#336): the documented plugin API, https://opencode.ai/docs/plugins/.
 *
 * The module's only export is a default `PluginModule`, so the pinned host calls
 * `server` and nothing else (probe `load.module-default`); it would otherwise call
 * every exported function as a plugin (`load.helper-export`). Hooks come from
 * `createTooluHooks`, which never rejects; the catch below guards the remaining
 * binding step so init can never fail open.
 */
import type { Plugin, PluginModule } from "@opencode-ai/plugin";
import { createDenyAllToolBefore } from "../adapter/tool-before.ts";
import { bindHostContext } from "./context.ts";
import { createTooluHooks } from "./hooks.ts";

const server: Plugin = async (input, options) => {
  try {
    return await createTooluHooks(bindHostContext(input, options, process.env));
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    return {
      "tool.execute.before": createDenyAllToolBefore(`toolu: not ready: ${reason}`),
      dispose: () => Promise.resolve(),
    };
  }
};

const tooluModule: PluginModule = { id: "toolu", server };

export default tooluModule;
