// CLI to read and edit the dashboard's machine config (dashboard.json). Pure
// transforms (setKey/addRoot/rmRoot) are exported and tested; runCli loads,
// applies one command, and saves. The dashboard-config slash command wraps this.
//
//   bun run config-cli.ts get
//   bun run config-cli.ts add-root ~/Projects
//   bun run config-cli.ts rm-root ~/Projects
//   bun run config-cli.ts set pollMs 1000
//   (append --path <file> to target a non-default config)

import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

import { type DashboardConfig, DEFAULT_CONFIG_PATH, expandHome, loadConfig } from "./config.ts";

const NUMERIC_FIELDS = ["scanDepth", "activeWithinHours", "stuckThresholdSeconds", "agentStuckSeconds", "pollMs", "port"];
const BOOL_FIELDS = ["open"];

/** Set a scalar config key, coercing/validating by key. Throws on unknown key or bad value. */
export function setKey(config: DashboardConfig, key: string, value: string): DashboardConfig {
  if (NUMERIC_FIELDS.includes(key)) {
    const n = Number(value);
    if (Number.isNaN(n)) throw new Error(`${key} must be a number, got "${value}"`);
    return { ...config, [key]: n };
  }
  if (BOOL_FIELDS.includes(key)) {
    if (value !== "true" && value !== "false") throw new Error(`${key} must be true|false`);
    return { ...config, [key]: value === "true" };
  }
  throw new Error(`unknown or non-scalar key "${key}"; settable: ${[...NUMERIC_FIELDS, ...BOOL_FIELDS].join(", ")}`);
}

/** Add a root (~-expanded, deduped). */
export function addRoot(config: DashboardConfig, dir: string): DashboardConfig {
  const abs = expandHome(dir);
  if (config.roots.includes(abs)) return config;
  return { ...config, roots: [...config.roots, abs] };
}

/** Remove a root (matching after ~-expansion). */
export function rmRoot(config: DashboardConfig, dir: string): DashboardConfig {
  const abs = expandHome(dir);
  return { ...config, roots: config.roots.filter((r) => r !== abs) };
}

/** Write config as pretty JSON, creating the parent dir. */
export function saveConfig(path: string, config: DashboardConfig): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, `${JSON.stringify(config, null, 2)}\n`);
}

/** Apply one CLI command against the config at `path`. Returns a human message. */
export function runCli(argv: string[], path: string = DEFAULT_CONFIG_PATH): { message: string; changed: boolean } {
  const [cmd, ...rest] = argv;
  const { config } = loadConfig(path);
  switch (cmd) {
    case "get":
      return { message: JSON.stringify(config, null, 2), changed: false };
    case "set": {
      if (rest.length < 2) throw new Error("usage: set <key> <value>");
      const next = setKey(config, rest[0], rest[1]);
      saveConfig(path, next);
      return { message: `set ${rest[0]} = ${rest[1]}`, changed: true };
    }
    case "add-root": {
      if (!rest[0]) throw new Error("usage: add-root <dir>");
      const next = addRoot(config, rest[0]);
      saveConfig(path, next);
      return { message: `roots: ${next.roots.join(", ") || "(none)"}`, changed: true };
    }
    case "rm-root": {
      if (!rest[0]) throw new Error("usage: rm-root <dir>");
      const next = rmRoot(config, rest[0]);
      saveConfig(path, next);
      return { message: `roots: ${next.roots.join(", ") || "(none)"}`, changed: true };
    }
    default:
      throw new Error(`unknown command "${cmd ?? ""}"; use get | set | add-root | rm-root`);
  }
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  let path = DEFAULT_CONFIG_PATH;
  const pIdx = args.indexOf("--path");
  if (pIdx >= 0) {
    path = args[pIdx + 1];
    args.splice(pIdx, 2);
  }
  try {
    console.log(runCli(args, path).message);
  } catch (err) {
    console.error(err instanceof Error ? err.message : String(err));
    process.exit(1);
  }
}
