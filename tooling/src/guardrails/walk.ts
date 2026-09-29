/**
 * Tree walks with `find` semantics: symlinks are never followed nor listed
 * (`-type f` / `-type d`), dot entries are included, and anything under a
 * `node_modules/` is filtered (`! -path '*\/node_modules/*'`) while a
 * `node_modules` directory itself is still listed. Results are sorted, so
 * violation order no longer depends on directory order.
 */
import { readdirSync, statSync } from "node:fs";
import { resolve } from "node:path";
import type { Dirent } from "node:fs";

type Entry = { rel: string; dirent: Dirent };

function children(root: string, rel: string): Entry[] {
  let entries: Dirent[];
  try {
    entries = readdirSync(resolve(root, rel), { withFileTypes: true });
  } catch {
    // find reports an unreadable directory on stderr (discarded) and moves on.
    return [];
  }
  return entries.map((dirent) => ({ rel: `${rel}/${dirent.name}`, dirent }));
}

function walk(root: string, start: string, visit: (entry: Entry) => void): void {
  const pending = children(root, start);
  while (pending.length > 0) {
    const entry = pending.pop();
    if (entry === undefined) break;
    visit(entry);
    if (entry.dirent.isDirectory() && entry.dirent.name !== "node_modules") {
      pending.push(...children(root, entry.rel));
    }
  }
}

/** Regular files under `dir` (relative to `root`), as `dir/…` paths. */
export function findFiles(root: string, dir: string): string[] {
  if (!isDir(root, dir)) return [];
  const out: string[] = [];
  walk(root, dir, (entry) => {
    if (entry.dirent.isFile()) out.push(entry.rel);
  });
  return out.toSorted();
}

/** Directories strictly under `dir`, as `dir/…` paths. */
export function findDirs(root: string, dir: string): string[] {
  if (!isDir(root, dir)) return [];
  const out: string[] = [];
  walk(root, dir, (entry) => {
    if (entry.dirent.isDirectory()) out.push(entry.rel);
  });
  return out.toSorted();
}

/** `[ -d path ]`: follows symlinks. */
export function isDir(root: string, rel: string): boolean {
  try {
    return statSync(resolve(root, rel)).isDirectory();
  } catch {
    return false;
  }
}

/** `[ -f path ]`: follows symlinks. */
export function isFile(root: string, rel: string): boolean {
  try {
    return statSync(resolve(root, rel)).isFile();
  } catch {
    return false;
  }
}

/** `[ -e path ]`: follows symlinks. */
export function exists(root: string, rel: string): boolean {
  try {
    statSync(resolve(root, rel));
    return true;
  } catch {
    return false;
  }
}
