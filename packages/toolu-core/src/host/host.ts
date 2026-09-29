/**
 * `@toolu/core/host` (#252): the one place that knows which host spawned a hook,
 * where that host keeps its roots, what it calls each event, and what output it
 * accepts. TypeScript port of `plugins/toolu/hooks/lib/host.sh`, extended from
 * Claude and Codex to Cursor, OpenCode and Hermes.
 *
 * Every function that reads the environment takes it explicitly (default
 * `process.env`); an empty variable counts as unset, like bash `${VAR:-}`. The
 * event map and encoders are pure.
 */
export { detectHost, type DetectOptions } from "./host-detect.ts";
export {
  HOST_EVENTS,
  canonicalEvent,
  hostsForNativeEvent,
  nativeEventName,
} from "./host-events.ts";
export type { HostEvent } from "./host-events.ts";
export { HOST_NAMES, envValue } from "./host-name.ts";
export type { HostEnv, HostName } from "./host-name.ts";
export {
  configRoot,
  invocation,
  pluginData,
  pluginInstallCommand,
  pluginRoot,
  projectConfigPath,
  projectDirname,
  projectRoot,
  projectStateDir,
  projectStateRoot,
  type HostOptions,
} from "./host-roots.ts";
export {
  codexPluginInstalled,
  codexPluginSnapshotPath,
  snapshotCodexPlugins,
  type CodexPluginSnapshot,
  type SnapshotResult,
} from "./host-snapshot.ts";
export {
  degradeAsk,
  encodeDecision,
  supportsAsk,
  type EncodedOutput,
  type GateClass,
} from "./host-encode.ts";
