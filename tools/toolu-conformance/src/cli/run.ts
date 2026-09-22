/** Conformance CLI — fixture matrix over real bash / bridge / bootstrap (#212). */
import { formatSuiteLine, runConformanceMatrix } from "./matrix.ts";

async function main(): Promise<void> {
  const { results, pass } = await runConformanceMatrix();
  for (const { id, outcome } of results) {
    process.stdout.write(`${formatSuiteLine(id, outcome)}\n`);
  }
  if (!pass) {
    process.exit(1);
  }
}

if (import.meta.main) {
  main().catch((err: unknown) => {
    const message = err instanceof Error ? err.message : String(err);
    console.error(`toolu-conformance: ${message}`);
    process.exit(1);
  });
}

export { runConformanceMatrix, runProtectedFilesConformance } from "./matrix.ts";
