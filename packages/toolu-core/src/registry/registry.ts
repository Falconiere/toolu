/**
 * `@toolu/core/registry` (#257): cross-plugin hook modules as bundled ESM files.
 * A plugin's SessionStart `register` copies its committed bundle into
 * `<config>/toolu/<dir>.d/<spec>__<name>.js`, and the core dispatcher imports
 * every such module in lexical order, gated on the owning plugin being
 * installed, isolating each module's failures. Replaces the assembled bash
 * fragments of `registry.sh`, `dispatch.sh` and the plugins' `register.sh`.
 */
export {
  REGISTRY_DIRS,
  registryEventDir,
  registryFileName,
  registryRoot,
} from "./registry-paths.ts";
export {
  REGISTRY_EVENTS,
  defineRegistryModule,
  registryEventFor,
  type RegistryContext,
  type RegistryEvent,
  type RegistryHookEvent,
  type RegistryModule,
} from "./registry-types.ts";
