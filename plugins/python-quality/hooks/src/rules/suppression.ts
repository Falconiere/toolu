/**
 * Forbidden suppression markers (#266, 30-suppression): bare `except:`, a
 * one-line `except …: pass`, blanket `# noqa` and blanket `# type: ignore`.
 * Each line reports its first matching form, and the first five are shown.
 */
import { ere, head, type PyFile } from "./py-file.ts";

const BARE = ere("^[[:space:]]*except[[:space:]]*:[[:space:]]*(#.*)?$");
const PASS = ere("^[[:space:]]*except[^:]*:[[:space:]]*pass[[:space:]]*(#.*)?$");
const NOQA = /#[ \t]*noqa/;
const SCOPED_NOQA = /#[ \t]*noqa[ \t]*:[ \t]*[A-Za-z0-9]/;
const IGNORE = /#[ \t]*type:[ \t]*ignore/;
const SCOPED_IGNORE = /#[ \t]*type:[ \t]*ignore\[/;

function form(line: string): string | undefined {
  if (BARE.test(line)) return "bare except: (swallows everything)";
  if (PASS.test(line)) return "one-line except ...: pass (silently discarded)";
  if (NOQA.test(line) && !SCOPED_NOQA.test(line)) return "blanket # noqa (no :CODE suffix)";
  if (IGNORE.test(line) && !SCOPED_IGNORE.test(line)) {
    return "blanket # type: ignore (no [code] suffix)";
  }
  return undefined;
}

export function suppression(f: PyFile): string | undefined {
  const hits = f.lines.flatMap((line, index) => {
    const found = form(line);
    return found === undefined ? [] : [`${String(index + 1)}: ${found}`];
  });
  if (hits.length === 0) return undefined;
  return `Forbidden suppression in ${f.file.path} — remove it and fix the underlying issue. Scoped forms (except SpecificError:, # noqa: E501, # type: ignore[arg-type]) are fine; blanket ones are not:\n${head(hits, 5)}`;
}
