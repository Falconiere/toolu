/**
 * One run of each tool with the legacy exemptions lifted, reduced to what the
 * stale check asks: which (file, rule) pairs oxlint reports, which files jscpd
 * finds in a clone, which files knip has a finding for. A crash or an
 * unreadable report is fatal: a failed run is never "no findings".
 */
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { z } from "zod";
import { fatal } from "./reach-schema.ts";

const OxlintReport = z.object({
  diagnostics: z.array(z.object({ code: z.string(), filename: z.string() })),
});
const ClonedFile = z.object({ name: z.string() });
const JscpdReport = z.object({
  duplicates: z.array(z.object({ firstFile: ClonedFile, secondFile: ClonedFile })),
});
const KnipReport = z.object({
  files: z.array(z.string()).default([]),
  issues: z.array(z.looseObject({ file: z.string() })),
});

/** stdout of a run that exited 0 (clean) or 1 (findings); anything else is fatal. */
function output(root: string, name: string, args: readonly string[], cwd: string): string {
  const bin = Bun.which(name) ?? join(root, "node_modules/.bin", name);
  const res = spawnSync(bin, args, { cwd, encoding: "utf8", maxBuffer: 256 * 1024 * 1024 });
  if (res.error !== undefined) fatal(`${name} did not run: ${res.error.message}`);
  if (res.status !== 0 && res.status !== 1) {
    const tail = res.stderr.trim().split("\n").slice(-5).join(" | ");
    fatal(`${name} exited ${String(res.status)}: ${tail}`);
  }
  return res.stdout;
}

function report<T>(name: string, text: string, schema: z.ZodType<T>): T {
  let raw: unknown;
  try {
    raw = JSON.parse(text);
  } catch {
    return fatal(`${name} report is not JSON`);
  }
  const parsed = schema.safeParse(raw);
  if (!parsed.success) {
    fatal(`${name} report has an unexpected shape: ${z.prettifyError(parsed.error)}`);
  }
  return parsed.data;
}

/** A rule the way a config names it: no `eslint/` prefix, `typescript/` for its plugin. */
function normalizeRule(rule: string): string {
  return rule.replace(/^eslint\//, "").replace(/^@typescript-eslint\//, "typescript/");
}

/** oxlint reports `plugin(rule)`. */
function ruleOf(code: string): string {
  const match = /^([\w-]+)\((.+)\)$/.exec(code);
  if (match === null) return fatal(`oxlint report has a diagnostic with no rule: "${code}"`);
  return normalizeRule(`${match[1] ?? ""}/${match[2] ?? ""}`);
}

export function findingKey(file: string, rule: string): string {
  return `${file}\t${normalizeRule(rule)}`;
}

/**
 * The lifted config sits beside the real one so relative `extends`,
 * `ignorePatterns` and `overrides` globs keep their meaning; lint discovery
 * only reads files named `.oxlintrc.json`, so a leftover is inert.
 */
export function oxlintFindings(
  root: string,
  dir: string,
  targets: readonly string[],
  lifted: unknown,
): Set<string> {
  const name = `.oxlintrc.lifted-${String(process.pid)}.json`;
  writeFileSync(join(dir, name), JSON.stringify(lifted));
  try {
    const text = output(
      root,
      "oxlint",
      ["--type-aware", "-f", "json", "-c", name, ...targets],
      dir,
    );
    const { diagnostics } = report("oxlint", text, OxlintReport);
    return new Set(diagnostics.map((d) => findingKey(d.filename, ruleOf(d.code))));
  } finally {
    rmSync(join(dir, name), { force: true });
  }
}

/** Files jscpd finds in at least one clone. The lifted config stays in `root`: its paths are relative. */
export function jscpdClones(root: string, lifted: unknown): Set<string> {
  const config = join(root, `.jscpd.lifted-${String(process.pid)}.json`);
  const out = mkdtempSync(join(tmpdir(), "toolu-jscpd-"));
  writeFileSync(config, JSON.stringify(lifted));
  try {
    output(
      root,
      "jscpd",
      ["--config", config, "--reporters", "json", "--output", out, "--silent"],
      root,
    );
    let text: string;
    try {
      text = readFileSync(join(out, "jscpd-report.json"), "utf8");
    } catch {
      return fatal("jscpd wrote no report");
    }
    const { duplicates } = report("jscpd", text, JscpdReport);
    return new Set(duplicates.flatMap((clone) => [clone.firstFile.name, clone.secondFile.name]));
  } finally {
    rmSync(config, { force: true });
    rmSync(out, { recursive: true, force: true });
  }
}

/** Files knip reports unused, or with at least one unused export, type or member. */
export function knipFindings(root: string, lifted: unknown): Set<string> {
  const dir = mkdtempSync(join(tmpdir(), "toolu-knip-"));
  const config = join(dir, "knip.json");
  writeFileSync(config, JSON.stringify(lifted));
  try {
    const text = output(
      root,
      "knip",
      ["--no-progress", "--reporter", "json", "--config", config],
      root,
    );
    const { files, issues } = report("knip", text, KnipReport);
    const withFinding = issues.filter((issue) =>
      Object.values(issue).some((value) => Array.isArray(value) && value.length > 0),
    );
    return new Set([...files, ...withFinding.map((issue) => issue.file)]);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
