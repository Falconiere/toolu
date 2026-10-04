import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
}

export function tmpBase(): string {
  return process.env.TMPDIR ?? "/tmp";
}

export function isolatedHome(prefix: string): string {
  return mkdtempSync(join(tmpBase(), prefix));
}

export function bridgeEnvClaude(root: string): Record<string, string> {
  return {
    TOOLU_SETTINGS_DIR: join(root, "plugins/toolu/settings"),
    TOOLU_HOST_OVERRIDE: "claude",
  };
}

export type ProtectedProject = {
  projectRoot: string;
  envPath: string;
  envBefore: string;
};

function writeProtectedProjectLayout(projectRoot: string): {
  envPath: string;
  envBefore: string;
} {
  const envPath = join(projectRoot, ".env");
  const envBefore = "SECRET=1\n";
  writeFileSync(envPath, envBefore, "utf8");
  mkdirSync(join(projectRoot, ".claude"), { recursive: true });
  writeFileSync(
    join(projectRoot, ".claude", "toolu.config.json"),
    JSON.stringify({
      version: 1,
      gates: { protectedFiles: { mode: "block" } },
    }),
    "utf8",
  );
  return { envPath, envBefore };
}

/** A `.env` under protectedFiles block mode, configured through the Claude project config. */
export function installProtectedProjectAt(projectRoot: string): ProtectedProject {
  const { envPath, envBefore } = writeProtectedProjectLayout(projectRoot);
  return { projectRoot, envPath, envBefore };
}

export function createProtectedProject(tempPrefix: string): ProtectedProject {
  const projectRoot = mkdtempSync(join(tmpBase(), tempPrefix));
  return installProtectedProjectAt(projectRoot);
}

export function readProtectedEnv(envPath: string): string {
  return readFileSync(envPath, "utf8");
}
