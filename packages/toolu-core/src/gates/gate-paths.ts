/**
 * Paths as the settings-driven gates compare them (#260): `repoRelative` is
 * `to_relative_path` from `detect.sh`, and `expandPattern` is bash pathname
 * expansion of a write target such as `.en[v]`, which writes whichever
 * existing file it matches, or the literal name when none does.
 */
import { readdirSync } from "node:fs";
import { join } from "node:path";
import { compileBashPattern } from "./bash-pattern.ts";

/** `to_relative_path`: `path` without the `<root>/` prefix; unchanged outside it or without a root. */
export function repoRelative(path: string, root: string | undefined): string {
  if (root === undefined || root === "") return path;
  return path.startsWith(`${root}/`) ? path.slice(root.length + 1) : path;
}

/** An unescaped `*`, `?`, bracket expression or extglob operator. */
function globs(segment: string): boolean {
  const unescaped = segment.replace(/\\./gs, "");
  return /[*?]/.test(unescaped) || /\[[^\]]*\]/.test(unescaped) || /[@!+]\(/.test(unescaped);
}

function unescape(segment: string): string {
  return segment.replace(/\\(.)/gs, "$1");
}

/** A directory's entries, sorted so the first hit is deterministic; none when it cannot be read. */
function entries(dir: string): string[] {
  try {
    return readdirSync(dir).toSorted();
  } catch {
    return [];
  }
}

/**
 * Every path `pattern` names under `cwd`, as bash globs it with `dotglob` off
 * (a leading `.` must be matched literally): each existing match, written the
 * way the pattern wrote its directories, then the pattern itself. Nothing is
 * capped: `tee *`, `sed -i … *` and `cp x *` write every match.
 */
export function expandPattern(pattern: string, cwd: string): string[] {
  const absolute = pattern.startsWith("/");
  const segments = (absolute ? pattern.slice(1) : pattern).split("/");
  let found: { shown: string[]; onDisk: string }[] = [{ shown: [], onDisk: absolute ? "/" : cwd }];
  for (const segment of segments) {
    if (!globs(segment)) {
      const name = unescape(segment);
      found = found.map((f) => ({ shown: [...f.shown, name], onDisk: join(f.onDisk, name) }));
      continue;
    }
    const match = compileBashPattern(segment);
    const dotted = segment.startsWith(".");
    found = found.flatMap((f) =>
      entries(f.onDisk)
        .filter((name) => (dotted || !name.startsWith(".")) && match(name))
        .map((name) => ({ shown: [...f.shown, name], onDisk: join(f.onDisk, name) })),
    );
  }
  const paths = found.map((f) => `${absolute ? "/" : ""}${f.shown.join("/")}`);
  return [...new Set([...paths, pattern])];
}
