/** Plugin inventory tri-state (#211). */

export type PluginInstallState = "installed" | "absent" | "unknown";

export type PluginEnabledState = "enabled" | "disabled" | "unknown";

export type PluginInventoryEntry = {
  name: string;
  spec: string;
  pluginDir: string;
  install: PluginInstallState;
  enabled: PluginEnabledState;
};

export type PluginManifest = {
  name: string;
  spec: string;
  marketplace: string;
  version: string;
  pluginDir: string;
  dependencies: Array<{ name: string; marketplace: string }>;
};
