/** Bootstrap output proof — exit 0 alone is not ready (#211). */
import { existsSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { opencodeRegistryRoot } from "../host/roots.ts";

const REGISTRY_EVENT_DIRS = ["pre-tools.d", "post-tools.d"] as const;

const SESSION_ARTIFACT_NAMES = [".gate-preset-notice-v6"] as const;

function listShModules(dir: string): string[] {
  if (!existsSync(dir)) {
    return [];
  }
  const out: string[] = [];
  for (const entry of readdirSync(dir)) {
    const path = join(dir, entry);
    try {
      if (statSync(path).isFile() && entry.endsWith(".sh")) {
        out.push(path);
      }
    } catch {
      continue;
    }
  }
  return out;
}

/** Concrete artifacts produced by register/session-start under dataRoot. */
export function collectBootstrapArtifacts(dataRoot: string): string[] {
  const regRoot = opencodeRegistryRoot(dataRoot);
  const artifacts: string[] = [];
  for (const sub of REGISTRY_EVENT_DIRS) {
    artifacts.push(...listShModules(join(regRoot, sub)));
  }
  for (const name of SESSION_ARTIFACT_NAMES) {
    const path = join(regRoot, name);
    if (existsSync(path)) {
      artifacts.push(path);
    }
  }
  return artifacts;
}

export function evaluateBootstrapReadiness(dataRoot: string): {
  ready: boolean;
  artifacts: string[];
  reason?: string;
} {
  const artifacts = collectBootstrapArtifacts(dataRoot);
  if (artifacts.length === 0) {
    return {
      ready: false,
      artifacts,
      reason:
        "bootstrap entrypoints exited cleanly but required registry/session artifacts are missing under the OpenCode data root",
    };
  }
  return { ready: true, artifacts };
}
