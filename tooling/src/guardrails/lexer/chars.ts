/**
 * Character helpers with bash substring semantics: an index past either end
 * reads as the empty string instead of undefined.
 */

export function at(text: string, index: number): string {
  return index >= 0 && index < text.length ? text.charAt(index) : "";
}

export function sub(text: string, index: number, length: number): string {
  return index >= 0 && index < text.length ? text.slice(index, index + length) : "";
}

/** `[ -z "${c//[[:space:]]/}" ]`: whitespace, or no character at all. */
export function isSpace(char: string): boolean {
  return char === "" || /^[ \t\n\v\f\r]$/.test(char);
}

/** `${s%"${s##*[![:space:]]}"}`: drop trailing whitespace. */
export function trimEnd(text: string): string {
  return text.replace(/[ \t\n\v\f\r]+$/, "");
}

/** `[[:alnum:]_$]` — an identifier character. */
export function isIdent(char: string): boolean {
  return /^[\p{L}\p{Nd}_$]$/u.test(char);
}

/** `[[:alpha:]_$]` — an identifier start. */
export function isIdentStart(char: string): boolean {
  return /^[\p{L}_$]$/u.test(char);
}

/** `[[:alnum:]_$.#]` — a character that makes a keyword a member or identifier suffix. */
export function isMemberChar(char: string): boolean {
  return /^[\p{L}\p{Nd}_$.#]$/u.test(char);
}

/** `[[:alpha:]_]` — what may follow `<` in a JSX tag. */
export function isTagStart(char: string): boolean {
  return /^[\p{L}_]$/u.test(char);
}
