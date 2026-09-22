/** Shared temp-repo setup for real bash bridge tests. */
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { join } from "node:path";

export const REPO_ROOT = join(import.meta.dir, "../../../..");

export async function createProtectedFilesProject(): Promise<{
  projectRoot: string;
  envPath: string;
}> {
  const base = process.env.TMPDIR ?? "/tmp";
  const projectRoot = await mkdtemp(join(base, "toolu-bridge-"));
  const envPath = join(projectRoot, ".env");
  await writeFile(envPath, "SECRET=1\n", "utf8");
  await mkdir(join(projectRoot, ".claude"), { recursive: true });
  await writeFile(
    join(projectRoot, ".claude/toolu.config.json"),
    JSON.stringify({
      version: 1,
      gates: { protectedFiles: { mode: "block" } },
    }),
    "utf8",
  );
  return { projectRoot, envPath };
}

export function bridgeEnv(): Record<string, string> {
  return {
    TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
    TOOLU_HOST_OVERRIDE: "claude",
  };
}
