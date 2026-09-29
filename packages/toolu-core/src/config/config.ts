/**
 * `@toolu/core/config` (#253): toolu.config.json loading and resolution, gate
 * modes, the host permission write and the plugin settings files. TypeScript
 * port of `plugins/toolu/hooks/lib/{config,quality-config,docs-sync-config,
 * gate-mode,permissions}.sh`; the bash libs stay until #279 and parity tests run
 * both over the same files.
 */
export { TooluConfigSchema, parseTooluConfig, type TooluConfig } from "./config-schema.ts";
export {
  configExists,
  isJsonObject,
  loadConfig,
  mergeConfig,
  type ConfigOptions,
  type JsonObject,
  type LoadedConfig,
  type Warn,
} from "./config-load.ts";
export {
  CODEX_REASONING_EFFORTS,
  MODEL_ALIASES,
  MODEL_CLASSES,
  codexModel,
  configString,
  enabled,
  enabledExplicit,
  flagFalse,
  flagTrue,
  model,
  section,
  type CodexModel,
  type ModelAlias,
  type ModelClass,
  type ReasoningEffort,
} from "./config-read.ts";
export {
  QUALITY_DEFAULTS,
  nativeMaxLines,
  qualityFlag,
  qualityThreshold,
  tsMaxFileLinesResolved,
  type QualityKey,
  type QualityLang,
  type QualityOptions,
  type ThresholdSource,
} from "./quality-config.ts";
export {
  DOCS_SYNC_DEFAULTS,
  docsSyncCodeSurfaces,
  docsSyncSurfaceExcludes,
  docsSyncSurfaces,
} from "./docs-sync-config.ts";
export {
  DEFAULT_GATE_PRESET,
  GATE_GUARDRAILS,
  GATE_MODES,
  GATE_NAMES,
  GATE_PRESETS,
  gateDecision,
  gateMode,
  gatePreset,
  guardrailWarning,
  type GateMode,
  type GateModeOptions,
  type GateName,
  type GatePreset,
} from "./gate-mode.ts";
