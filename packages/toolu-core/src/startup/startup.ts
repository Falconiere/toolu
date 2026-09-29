/**
 * `@toolu/core/startup` (#269): what the small SessionStart hooks share —
 * publishing a plugin file at a stable config-root path, the Bun-on-PATH
 * advisory, bounded context output, and Codex plugin-dependency warnings with
 * host-native install commands. Ports the `session-start.sh`,
 * `check-toolu.sh` and `check-deps.sh` scripts of the leaf plugins.
 */
export {
  MAX_CONTEXT_CHARS,
  renderHookOutput,
  sessionContext,
  type SessionContext,
} from "./context.ts";
export {
  CORE_PLUGIN,
  codexDependencyNotice,
  codexMissingPlugins,
  requiresCoreWarning,
  requiresPluginsWarning,
} from "./dependencies.ts";
export {
  bunAdvisory,
  bunOnPath,
  publishWrapper,
  type PublishOptions,
  type PublishResult,
} from "./publish.ts";
