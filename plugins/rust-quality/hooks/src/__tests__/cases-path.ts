/**
 * PATH variations for the golden cases (#267): the current PATH minus some
 * tools (one directory of symlinks to every other command), or with a stub
 * `ast-grep` first, as the bats suites did.
 */
import { chmodSync, existsSync, mkdirSync, readdirSync, symlinkSync, writeFileSync } from "node:fs";
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

/** PATH with a stub `ast-grep` running `body` first. */
export function stubAstGrep(body: string): (sb: Sandbox) => { PATH: string } {
  return (sb) => {
    const bin = join(sb.root, "stub-bin");
    mkdirSync(bin, { recursive: true });
    writeFileSync(join(bin, "ast-grep"), `#!/bin/sh\n${body}\n`);
    chmodSync(join(bin, "ast-grep"), 0o755);
    return { PATH: `${bin}:${process.env.PATH ?? ""}` };
  };
}
