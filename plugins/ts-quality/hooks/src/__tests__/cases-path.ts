/**
 * A PATH that lacks some tools (#265): one directory of symlinks to every
 * command on the current PATH except `without`, so a case sees the real
 * environment minus exactly those tools (ast-grep absent, jq absent).
 */
import { existsSync, mkdirSync, readdirSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";

function commands(dir: string): string[] {
  try {
    return readdirSync(dir);
  } catch {
    return [];
  }
}

/** PATH over a fresh directory holding every PATH command but `without`. */
export function pathWithout(...without: string[]): (sb: Sandbox) => { PATH: string } {
  return (sb) => {
    const bin = join(sb.root, `path-without-${without.join("-")}`);
    mkdirSync(bin, { recursive: true });
    for (const dir of (process.env.PATH ?? "").split(":")) {
      if (dir === "") continue;
      for (const name of commands(dir)) {
        const link = join(bin, name);
        if (without.includes(name) || existsSync(link)) continue;
        symlinkSync(join(dir, name), link);
      }
    }
    return { PATH: bin };
  };
}
