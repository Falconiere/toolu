/** Conformance CLI — protected-files bridge against real pre-tools/mod.sh (#210). */
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { runPreToolBridge } from "@toolu/core/bridge";

export type ConformanceResult = { pass: true } | { pass: false; message: string };

function repoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../../..");
}

/** Run protected-files pre-tool conformance; real bash only. */
export async function runProtectedFilesConformance(): Promise<ConformanceResult> {
  const root = repoRoot();
  const fixturePath = join(root, "tooling/fixtures/portable-core/protected-files-pre.json");
  const fixtureRaw: unknown = JSON.parse(readFileSync(fixturePath, "utf8"));

  const tmpBase = process.env.TMPDIR ?? "/tmp";
  const projectRoot = mkdtempSync(join(tmpBase, "toolu-conformance-"));
  const envPath = join(projectRoot, ".env");
  writeFileSync(envPath, "SECRET=1\n", "utf8");
  mkdirSync(join(projectRoot, ".claude"), { recursive: true });
  writeFileSync(
    join(projectRoot, ".claude/toolu.config.json"),
    JSON.stringify({
      version: 1,
      gates: { protectedFiles: { mode: "block" } },
    }),
    "utf8",
  );

  const request = {
    ...(typeof fixtureRaw === "object" && fixtureRaw !== null ? fixtureRaw : {}),
    cwd: projectRoot,
    projectRoot,
    worktree: projectRoot,
    toolInput: { file_path: envPath },
  };

  const response = await runPreToolBridge(request, {
    repoRoot: root,
    env: {
      TOOLU_SETTINGS_DIR: join(root, "plugins/toolu/settings"),
      TOOLU_HOST_OVERRIDE: "claude",
    },
  });

  if (!response.ok) {
    return {
      pass: false,
      message: `bridge failed: ${response.decision.kind} ${JSON.stringify(response.decision)}`,
    };
  }

  const kind = response.decision.kind;
  if (kind === "deny" || kind === "ask") {
    return { pass: true };
  }

  return {
    pass: false,
    message: `expected deny or ask for protected .env edit, got ${kind}`,
  };
}

async function main(): Promise<void> {
  const result = await runProtectedFilesConformance();
  if (!result.pass) {
    console.error(`toolu-conformance: ${result.message}`);
    process.exit(1);
  }
  process.stdout.write("toolu-conformance: protected-files ok\n");
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`toolu-conformance: ${message}`);
    process.exit(1);
  });
}
