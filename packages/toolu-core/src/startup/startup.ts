/**
 * `@toolu/core/startup` (#269): what the small SessionStart hooks share —
 * publishing a plugin file at a stable config-root path, the Bun-on-PATH
 * advisory, bounded context output, Codex plugin-dependency warnings with
 * host-native install commands, and the startup report a self-hosting
 * bootstrap reads (#342), and the OpenCode status record (#359). Ports the `session-start.sh`, `check-toolu.sh` and
 * `check-deps.sh` scripts of the leaf plugins.
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
  publishBunCli,
  publishWrapper,
  type PublishOptions,
  type PublishResult,
} from "./publish.ts";
export {
  STARTUP_REPORT_ENV,
  reportStartup,
  type ErrorStartupRecord,
  type HelperStartupRecord,
  type RegistryStartupRecord,
  type StartupRecord,
} from "./report.ts";
export {
  MAX_OPENCODE_STATUS_BYTES,
  OPENCODE_STATUS_FILE,
  opencodeStatusPath,
  parseOpencodeStatus,
  readOpencodeStatus,
  type OpencodeStatusPlugin,
  type OpencodeStatusRead,
  type OpencodeStatusRecord,
} from "./opencode-status.ts";
