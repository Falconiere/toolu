/** The first non-blank line of a command's output, or a fallback when there is none. */
export function firstLine(text: string, fallback: string): string {
  return (
    text
      .split("\n")
      .find((line) => line.trim().length > 0)
      ?.trim() ?? fallback
  );
}
