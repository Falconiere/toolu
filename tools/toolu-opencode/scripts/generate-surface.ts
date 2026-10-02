/** Generate committed OpenCode skill/agent/command surface (#206). */
import { existsSync, mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { selectPluginsByEnabledNames } from "../src/select/resolve.ts";
import { listPluginManifests } from "../src/inventory/scan.ts";
import { GENERATED_SEGMENT } from "./lib/constants.ts";
import { planSurface, readTree, treesEqual, writeSurface } from "./lib/emit.ts";

const CliSchema = z.object({
  repoRoot: z.string().min(1).optional(),
  enabled: z.array(z.string().min(1)).optional(),
  outDir: z.string().min(1).optional(),
  check: z.boolean().optional(),
});

function defaultRepoRoot(): string {
  return join(dirname(fileURLToPath(import.meta.url)), "../../..");
}

function parseArgs(argv: string[]): z.infer<typeof CliSchema> {
  const out: Record<string, unknown> = {};
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "--check") {
      out.check = true;
      continue;
    }
    if (arg === "--repo" && argv[i + 1]) {
      out.repoRoot = argv[i + 1];
      i += 1;
      continue;
    }
    if (arg === "--out" && argv[i + 1]) {
      out.outDir = argv[i + 1];
      i += 1;
      continue;
    }
    if (arg === "--enabled" && argv[i + 1]) {
      out.enabled = argv[i + 1]
        .split(",")
        .map((s) => s.trim())
        .filter(Boolean);
      i += 1;
    }
  }
  return CliSchema.parse(out);
}

export function runGenerateSurface(argv: string[] = process.argv.slice(2)): number {
  const cli = parseArgs(argv);
  const repoRoot = resolve(cli.repoRoot ?? defaultRepoRoot());
  const outDir = resolve(cli.outDir ?? join(repoRoot, GENERATED_SEGMENT));
  const pluginsRoot = join(repoRoot, "plugins");

  const plugins = cli.enabled
    ? selectPluginsByEnabledNames(pluginsRoot, cli.enabled)
    : { ok: true as const, plugins: listPluginManifests(pluginsRoot) };
  if (!plugins.ok || !plugins.plugins) {
    console.error(plugins.ok ? `cannot read plugins root: ${pluginsRoot}` : plugins.reason);
    return 1;
  }

  const plan = planSurface({ repoRoot, outDir, plugins: plugins.plugins });

  if (cli.check) {
    if (!existsSync(outDir)) {
      console.error(`surface drift: missing generated directory ${outDir}`);
      return 1;
    }
    const onDisk = readTree(outDir);
    const diffs = treesEqual(plan.files, onDisk, outDir);
    if (diffs.length > 0) {
      for (const line of diffs) {
        console.error(`surface drift: ${line}`);
      }
      return 1;
    }
    return 0;
  }

  mkdirSync(outDir, { recursive: true });
  writeSurface(plan, outDir);
  return 0;
}

if (import.meta.main) {
  process.exit(runGenerateSurface());
}
