/**
 * Every tracked TypeScript file against every gate. An unreached file is a
 * finding unless tooling/gate-reach.json allows it for that tool, and an
 * allowance that covers no unreached file is a finding too, so the declared
 * gaps shrink when trees are brought in.
 */
import { matches, plainGlobs, trackedFiles } from "./reach-files.ts";
import { ReachConfigSchema, readJson } from "./reach-schema.ts";
import { loadReach } from "./reach-tools.ts";

export const REACH_CONFIG = "tooling/gate-reach.json";

export function checkReach(root: string): string[] {
  const config = readJson(root, REACH_CONFIG, ReachConfigSchema);
  plainGlobs(REACH_CONFIG, [...config.exclude, ...config.allowances.map(({ glob }) => glob)]);
  const reach = loadReach(root);
  const used = new Set<number>();
  const findings: string[] = [];
  for (const file of trackedFiles(root, config.exclude)) {
    for (const [tool, reached] of reach) {
      if (reached(file)) continue;
      const covering = config.allowances.flatMap((allowance, index) =>
        allowance.tool === tool && matches(allowance.glob, file) ? [index] : [],
      );
      if (covering.length === 0) findings.push(`gate-reach: ${file}: not reached by ${tool}`);
      for (const index of covering) used.add(index);
    }
  }
  config.allowances.forEach(({ tool, glob }, index) => {
    if (!used.has(index)) {
      findings.push(`gate-reach: stale allowance: ${tool} ${glob} covers no unreached file`);
    }
  });
  return findings;
}
