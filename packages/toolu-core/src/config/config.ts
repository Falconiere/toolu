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
