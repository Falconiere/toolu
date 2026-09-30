/**
 * rust-quality's PostToolUse registry module (#267): the post-edit Rust
 * quality rules, run in process by toolu's dispatcher for every edited `.rs`
 * file of a Cargo project with cargo on PATH. Violations fail the file's
 * quality-gate entry; the docs advisory only informs. Replaces the bash
 * concern fragments `register.sh` assembled.
 */
import { readFileSync } from "node:fs";
import {
  loadConfig,
  qualityFlag,
  qualityThreshold,
  rustUnsafeExemptions,
  settingsDir,
} from "@toolu/core/config";
import { detectRust, toolAvailable } from "@toolu/core/detect";
import { fileQuality, type EditedFile } from "@toolu/core/quality";
import { defineRegistryModule, type RegistryContext } from "@toolu/core/registry";
import { checkRsFile } from "./rules/check.ts";
import { splitLines, type RsLimits } from "./rules/rs-file.ts";

const ALLOW = { kind: "allow" } as const;

function limitsFor(ctx: RegistryContext): RsLimits {
  const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => undefined });
  const settings = settingsDir({ env: ctx.env });
  return {
    fileLines: qualityThreshold(config, "rust", "maxFileLines", where),
    fnLines: qualityThreshold(config, "rust", "maxFnLines", where),
    implLines: qualityThreshold(config, "rust", "maxImplLines", where),
    noMocks: qualityFlag(config, "rust", "noMocks", true),
    unsafeExemptions: settings === undefined ? [] : rustUnsafeExemptions(settings),
  };
}

function read(file: EditedFile): { text: string; error?: never } | { text?: never; error: string } {
  try {
    return { text: readFileSync(file.absolute, "utf8") };
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    return { error: `Cannot read ${file.path} for Rust quality checks: ${detail}` };
  }
}

export default defineRegistryModule({
  spec: "rust-quality@toolu",
  name: "rust-quality",
  event: "tool/post",
  run(event, ctx) {
    const where = { env: ctx.env, ...(ctx.cwd === undefined ? {} : { cwd: ctx.cwd }) };
    if (!detectRust(where) || !toolAvailable("cargo", ctx.env)) return Promise.resolve(ALLOW);
    const decision = fileQuality(event, ctx, {
      source: "rust-quality-hook",
      reason: "Post-edit Rust quality violation(s) detected",
      matches: /\.rs$/s,
      skipLinkedWorktrees: false,
      check: (file) => {
        const source = read(file);
        if (source.error !== undefined) return { errors: [source.error], advisories: [] };
        return checkRsFile({ file, lines: splitLines(source.text), ctx, limits: limitsFor(ctx) });
      },
    });
    return Promise.resolve(decision);
  },
});
