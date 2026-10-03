/** Worktree-local binding applies to ledger commands without inherited env. */
import { existsSync, readFileSync, realpathSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { isJsonObject } from "../config/config-load.ts";
import { readJsonFile, writeJsonAtomic } from "./lock.ts";

export type ResourceBinding = {
  version: 1;
  root: string;
  key: string;
  stateDir: string;
  worktree: string;
};

function gitDir(cwd: string): { git: string; worktree: string } | null {
  let path = realpathSync(cwd);
  for (;;) {
    const marker = join(path, ".git");
    if (existsSync(marker)) {
      if (statSync(marker).isDirectory()) return { git: marker, worktree: path };
      const target = /^gitdir: (.+)\s*$/.exec(readFileSync(marker, "utf8"))?.[1];
      if (!target) throw new Error(`invalid git worktree marker: ${marker}`);
      return { git: resolve(path, target.trim()), worktree: path };
    }
    const parent = dirname(path);
    if (parent === path) return null;
    path = parent;
  }
}

export function bindWorktree(root: string, worktree: string, key: string, stateDir: string): void {
  const target = gitDir(worktree);
  if (!target) throw new Error("cannot bind resources outside a Git worktree");
  writeJsonAtomic(join(target.git, "toolu-resource.json"), {
    version: 1,
    root,
    key,
    stateDir,
    worktree: target.worktree,
  });
}

export function resourceBinding(cwd: string): ResourceBinding | null {
  const target = gitDir(cwd);
  if (!target) return null;
  const binding = readJsonFile(join(target.git, "toolu-resource.json"), null);
  if (binding === null) return null;
  if (
    !isJsonObject(binding) ||
    binding.version !== 1 ||
    typeof binding.root !== "string" ||
    !binding.root.startsWith("/") ||
    binding.worktree !== target.worktree ||
    typeof binding.key !== "string" ||
    typeof binding.stateDir !== "string"
  )
    throw new Error("invalid worktree resource binding");
  return {
    version: 1,
    root: binding.root,
    worktree: binding.worktree,
    key: binding.key,
    stateDir: binding.stateDir,
  };
}
