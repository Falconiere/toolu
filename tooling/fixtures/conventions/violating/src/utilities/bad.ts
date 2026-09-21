/** Intentional violations for conventions fixture tests. */
export function bad(value: any): string {
  const cast = value as string;
  // oxlint-disable-next-line typescript/no-explicit-any
  const again: any = cast;
  return again;
}
