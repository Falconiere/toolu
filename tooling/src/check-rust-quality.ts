#!/usr/bin/env bun
/**
 * The rust-quality plugin's own rules over the whole Rust workspace (#455):
 * every listed `.rs` file under `crates/` through `checkRsFile`, with the
 * limits this repository configures in `.claude/toolu.config.json`. It is what
 * an agent sees after each edit, applied to every file; `cargo xtask gate`
 * runs it in its `rust-quality` step.
 *
 * Usage: bun run tooling/src/check-rust-quality.ts [root]
 */
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { loadConfig, qualityFlag, qualityThreshold } from "@toolu/core/config";
import type { RegistryContext } from "@toolu/core/registry";
import { checkRsFile } from "../../plugins/rust-quality/hooks/src/rules/check.ts";
import { splitLines, type RsLimits } from "../../plugins/rust-quality/hooks/src/rules/rs-file.ts";

/** Listed (tracked or untracked, not ignored) `.rs` files under `crates/`. */
export function rustFiles(root: string): string[] {
  const res = spawnSync(
    "git",
    ["-C", root, "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "crates"],
    { encoding: "utf8" },
  );
  if (res.error !== undefined) throw res.error;
  if (res.status !== 0) throw new Error(`git ls-files failed in ${root}: ${res.stderr.trim()}`);
  return res.stdout
    .split("\0")
    .filter((path) => path.endsWith(".rs"))
    .toSorted();
}

/** The Claude Code context of a hook running at `root`; both config layers are the repository's. */
function contextAt(root: string): RegistryContext {
  const env = { ...process.env, CLAUDE_CONFIG_DIR: join(root, ".claude") };
  return {
    host: "claude",
    env,
    configRoot: join(root, ".claude"),
    projectRoot: root,
    cwd: root,
    raw: {},
  };
}

/** The limits the post-edit hook resolves for `root`. */
export function limitsAt(ctx: RegistryContext): RsLimits {
  const where = { env: ctx.env, cwd: ctx.projectRoot };
  const config = loadConfig({ ...where, host: ctx.host, warn: () => undefined });
  return {
    fileLines: qualityThreshold(config, "rust", "maxFileLines", where),
    fnLines: qualityThreshold(config, "rust", "maxFnLines", where),
    implLines: qualityThreshold(config, "rust", "maxImplLines", where),
    noMocks: qualityFlag(config, "rust", "noMocks", true),
    unsafeExemptions: [],
  };
}

/** Every blocking rust-quality finding under `root`. */
export function checkTree(root: string): string[] {
  const ctx = contextAt(root);
  const limits = limitsAt(ctx);
  return rustFiles(root).flatMap((path) => {
    const absolute = join(root, path);
    const lines = splitLines(readFileSync(absolute, "utf8"));
    return checkRsFile({ file: { path, absolute, removed: false }, lines, ctx, limits }).errors;
  });
}

function main(args: readonly string[]): number {
  const root = resolve(args[0] ?? join(import.meta.dir, "../.."));
  const errors = checkTree(root);
  for (const error of errors) console.error(error);
  if (errors.length > 0) {
    console.error(`rust-quality: ${String(errors.length)} finding(s)`);
    return 1;
  }
  process.stdout.write(`rust-quality: ${String(rustFiles(root).length)} files, ok\n`);
  return 0;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
