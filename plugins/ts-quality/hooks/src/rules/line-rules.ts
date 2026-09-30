/**
 * The grep-based rules (#265), each a port of one bash fragment: `as`
 * assertions (15), React hooks (35), factories (40), type guards (45),
 * component file names (55), console.log (60), suppression comments (65),
 * confirm/alert (70), raw radix imports (72), mutable props (74) and
 * try/catch+toast (76). Each returns its `add_error` text, or undefined.
 */
import { readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";
import {
  countMatches,
  ere,
  head,
  isTestPath,
  numbered,
  pathHas,
  withExcerpt,
  withoutCommentLines,
  type TsFile,
} from "./ts-file.ts";

const AS_PATTERN = ere(
  String.raw`\)[[:space:]]+as[[:space:]]+[a-zA-Z]|\bas[[:space:]]+any\b|\bas[[:space:]]+unknown\b|[a-zA-Z>][[:space:]]+as[[:space:]]+[A-Z]|[a-zA-Z>][[:space:]]+as[[:space:]]+(string|number|boolean|object|symbol|bigint|never|undefined|null|void)\b`,
);
const AS_CONST = ere(String.raw`\bas[[:space:]]+const\b`);
const AS_IMPORT_OR_REEXPORT = ere(
  String.raw`\bimport\b|^[0-9]+:[[:space:]]*export[[:space:]]*(type[[:space:]]+)?\{`,
);

/** 15-type-as. */
export function typeAssertion(f: TsFile): string | undefined {
  const rows = withoutCommentLines(numbered(f.lines, AS_PATTERN)).filter(
    (row) => !AS_CONST.test(row) && !AS_IMPORT_OR_REEXPORT.test(row),
  );
  const header = `Forbidden 'as' type assertion in ${f.file.path} — use type guards or Zod`;
  return withExcerpt(header, head(rows, 5));
}

/** 35-react-hooks. */
export function reactHooks(f: TsFile): string | undefined {
  if (!/use-.*\.ts$/s.test(f.file.path) && !/use[A-Z].*\.ts$/s.test(f.file.path)) return undefined;
  const count = countMatches(
    f.lines,
    ere(String.raw`^[[:space:]]*(const \[|useRef\(|useEffect\()`),
  );
  if (count <= 3) return undefined;
  return `Hook does too many things in ${f.file.path} (${String(count)} useState/useRef/useEffect) — split into focused hooks`;
}

/** 40-factory. */
export function factories(f: TsFile): string | undefined {
  const count = countMatches(f.lines, /^export (async )?function create/s);
  if (count <= 2) return undefined;
  return `Too many factory functions in ${f.file.path} (${String(count)}) — simplify construction`;
}

function projectUsesZod(f: TsFile): boolean {
  const manifest = join(f.ctx.projectRoot, "package.json");
  try {
    return statSync(manifest).isFile() && readFileSync(manifest, "utf8").includes('"zod"');
  } catch {
    return false;
  }
}

/** 45-typeguard. */
export function manualTypeGuard(f: TsFile): string | undefined {
  if (countMatches(f.lines, /function is[A-Z].*\): .* is [A-Z]/s) === 0) return undefined;
  if (!projectUsesZod(f)) return undefined;
  return `Manual type guard in ${f.file.path} — use Zod schema instead`;
}

/** 55-naming: a `.tsx` named after a grab-bag, not its exported component. */
export function componentFileName(f: TsFile): string | undefined {
  if (!/\.(tsx)$/s.test(f.file.path)) return undefined;
  const name = basename(f.file.path)
    .replace(/\.tsx$/s, "")
    .replace(/\.ts$/s, "");
  const grabBag =
    /^(parts|components|helpers|items|sections|elements)$|-(parts|sections|items|elements)$/s;
  if (!grabBag.test(name)) return undefined;
  return `Forbidden component filename '${name}.tsx' in ${f.file.path} — name file after its exported function (e.g. api-key-create-button.tsx)`;
}

/** 60-console. */
export function consoleLog(f: TsFile): string | undefined {
  const rows = withoutCommentLines(numbered(f.lines, ere(String.raw`^[[:space:]]*console\.log\(`)));
  const header = `Forbidden console.log in ${f.file.path} — use console.error/warn/info`;
  return withExcerpt(header, head(rows, 3));
}

/** 65-suppression: `@ts-expect-error` is exempt only in test files. */
export function suppressionComment(f: TsFile): string | undefined {
  const tokens = isTestPath(f.file.path)
    ? "@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore"
    : "@ts-ignore|@ts-nocheck|eslint-disable|biome-ignore|@ts-expect-error";
  const rows = numbered(f.lines, ere(String.raw`(//|/\*+)[[:space:]]*(${tokens})`));
  const header = `Forbidden suppression comment in ${f.file.path} — fix the underlying issue in code, never silence it`;
  return withExcerpt(header, head(rows, 3));
}

function isFrontend(f: TsFile): boolean {
  return pathHas(f, "/components/") || pathHas(f, "/routes/");
}

/** 70-ui-confirm. */
export function confirmAlert(f: TsFile): string | undefined {
  if (!isFrontend(f)) return undefined;
  const shared = /(ConfirmDeleteAlert|AlertDialog|customAlert|customConfirm)/s;
  const rows = withoutCommentLines(
    numbered(f.lines, ere(String.raw`\b(confirm|alert)[[:space:]]*\(`)),
  ).filter((row) => !shared.test(row));
  const header = `Forbidden confirm()/alert() in ${f.file.path} — use AlertDialog component`;
  return withExcerpt(header, head(rows, 3));
}

/** 72-ui-radix. */
export function rawRadixImport(f: TsFile): string | undefined {
  const imports = numbered(f.lines, /from ['"]@radix-ui\/react-(alert-dialog|dialog)['"]/s);
  if (withoutCommentLines(imports).length === 0 || pathHas(f, "/packages/ui/")) return undefined;
  return `Raw radix import in ${f.file.path} — use shared components from @/components/ui/`;
}

/** 74-react-props. */
export function mutableProps(f: TsFile): string | undefined {
  const rows = numbered(
    f.lines,
    ere(String.raw`\((props|[a-z]+Props):[[:space:]]+[A-Z][a-zA-Z]+Props\)`),
  ).filter((row) => !row.includes("Readonly"));
  return withExcerpt(`Mutable props in ${f.file.path} — wrap in Readonly<Props>`, head(rows, 3));
}

/** 76-toast: a `toast(` after a `catch (`, in a component or route. */
export function catchToast(f: TsFile): string | undefined {
  if (!isFrontend(f)) return undefined;
  const opens = ere(String.raw`catch[[:space:]]*\(`);
  const rows: string[] = [];
  let found = false;
  f.lines.forEach((line, i) => {
    if (opens.test(line)) found = true;
    if (found && line.includes("toast(")) {
      rows.push(`${String(i + 1)}: ${line}`);
      found = false;
    }
  });
  const header = `Manual try/catch+toast in ${f.file.path} — use shared error handling`;
  return withExcerpt(header, head(rows, 3));
}
