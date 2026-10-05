/**
 * Changed files → group switches (#458). A file is release-only when its path
 * is in `releaseOnly.paths` and its diff touches only version lines (any
 * CHANGELOG.md change counts). Everything else turns on the groups it matches;
 * `runEverything` paths and paths no group matches turn every group on.
 */
import { CHANGED, type CiPaths, matchesAny, outputNames } from "./config.ts";

/** One changed path and the `+`/`-` lines of its diff (`git diff -U0`, headers dropped). */
export type ChangedFile = { path: string; lines: readonly string[] };

export type Classification = {
  /** Output name → on. Includes every group and `changed`. */
  outputs: Record<string, boolean>;
  /** Why the outputs are what they are, for the job log. */
  reasons: string[];
};

const CHANGELOG = "CHANGELOG.md";
const SEMVER = String.raw`\^?\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?`;

function escapeRegExp(text: string): string {
  return text.replace(/[.*+?^${}()|[\]\\]/g, String.raw`\$&`);
}

function versionLine(versionKeys: readonly string[]): RegExp {
  const keys = versionKeys.map(escapeRegExp).join("|");
  return new RegExp(String.raw`^[+-]\s*"(?:${keys})"\s*:\s*"${SEMVER}"\s*,?\s*$`);
}

/** Whether `file` is a release-please version bump the gates need not see. */
export function isReleaseOnly(config: CiPaths, file: ChangedFile): boolean {
  if (!matchesAny(config.releaseOnly.paths, file.path)) return false;
  if (file.path === CHANGELOG) return true;
  const pattern = versionLine(config.releaseOnly.versionKeys);
  return file.lines.length > 0 && file.lines.every((line) => pattern.test(line));
}

/** Every output on, with the reason logged. */
export function allOn(config: CiPaths, reason: string): Classification {
  const outputs = Object.fromEntries(outputNames(config).map((name) => [name, true]));
  return { outputs, reasons: [`every group on: ${reason}`] };
}

/** Classify a diff. An empty diff turns everything on: no real event produces one. */
export function classify(config: CiPaths, files: readonly ChangedFile[]): Classification {
  if (files.length === 0) return allOn(config, "the diff is empty");
  const outputs = Object.fromEntries(outputNames(config).map((name) => [name, false]));
  const reasons: string[] = [];
  for (const file of files) {
    if (isReleaseOnly(config, file)) {
      reasons.push(`${file.path}: release-only`);
      continue;
    }
    outputs[CHANGED] = true;
    if (matchesAny(config.runEverything, file.path)) {
      return allOn(config, `${file.path} runs everything`);
    }
    const groups = Object.entries(config.groups).filter(([, globs]) =>
      matchesAny(globs, file.path),
    );
    if (groups.length === 0) return allOn(config, `${file.path} matches no group`);
    for (const [name] of groups) outputs[name] = true;
    reasons.push(`${file.path}: ${groups.map(([name]) => name).join(", ")}`);
  }
  return { outputs, reasons };
}
