/**
 * Static word values (#284). A word is static only when every part is literal,
 * so `"$(cat <<'EOF' … EOF)"`, the form agents write commit messages in,
 * resolves to its heredoc body while anything that expands at run time is
 * `null`.
 */
import type { ParsedScript, Redirect, Word, WordPart } from "unbash";
import { unreachable, type ShellRedirect } from "./shell-types.ts";

/** A heredoc body's text as the command reads it, or `null` when it expands at run time. */
export function heredocContent(redirect: Redirect): string | null {
  const content = redirect.content ?? "";
  // An unquoted body expands `$`, backticks and backslash escapes: static only without them.
  if (redirect.heredocQuoted !== true && /[$`\\]/.test(content)) return null;
  return redirect.operator === "<<-" ? content.replace(/^\t+/gm, "") : content;
}

function isHeredoc(redirect: Redirect): boolean {
  return redirect.operator === "<<" || redirect.operator === "<<-";
}

/** `$(cat <<TAG … TAG)`: the body minus trailing newlines, as command substitution yields it. */
function heredocCat(script: ParsedScript | undefined): string | null {
  if (script === undefined || (script.errors?.length ?? 0) > 0) return null;
  const [statement, ...others] = script.commands;
  if (statement === undefined || others.length > 0 || statement.background === true) return null;
  const command = statement.command;
  if (command.type !== "Command" || statement.redirects.length > 0) return null;
  if (command.prefix.length > 0 || command.suffix.length > 0) return null;
  if (command.name === undefined || staticWord(command.name) !== "cat") return null;
  const [redirect, ...more] = command.redirects;
  if (redirect === undefined || more.length > 0 || !isHeredoc(redirect)) return null;
  return heredocContent(redirect)?.replace(/\n+$/, "") ?? null;
}

function staticQuoted(children: readonly WordPart[]): string | null {
  let value = "";
  for (const child of children) {
    const piece = child.type === "CommandExpansion" ? heredocCat(child.script) : staticPart(child);
    if (piece === null) return null;
    value += piece;
  }
  return value;
}

function staticPart(part: WordPart): string | null {
  switch (part.type) {
    case "Literal":
    case "SingleQuoted":
    case "AnsiCQuoted":
      return part.value;
    case "DoubleQuoted":
    case "LocaleString":
      return staticQuoted(part.parts);
    case "SimpleExpansion":
    case "ParameterExpansion":
    case "CommandExpansion":
    case "ArithmeticExpansion":
    case "ProcessSubstitution":
    case "ExtendedGlob":
    case "BraceExpansion":
      return null;
    default:
      return unreachable(part);
  }
}

/** The word's value when every part is literal, else `null`. */
export function staticWord(word: Word): string | null {
  const parts = word.parts;
  if (parts === undefined) return word.value;
  let value = "";
  for (const part of parts) {
    const piece = staticPart(part);
    if (piece === null) return null;
    value += piece;
  }
  return value;
}

export function toShellRedirect(redirect: Redirect): ShellRedirect {
  const heredoc = isHeredoc(redirect);
  return {
    operator: redirect.operator,
    fd: redirect.fileDescriptor ?? null,
    target: heredoc || redirect.target === undefined ? null : staticWord(redirect.target),
    text: heredoc ? "" : (redirect.target?.text ?? ""),
    heredoc: heredoc
      ? { content: heredocContent(redirect), quoted: redirect.heredocQuoted === true }
      : null,
  };
}
