/** Deterministic, OpenCode-compatible surface id assignment. */
import { createHash } from "node:crypto";
import type { ArtifactKind } from "./constants.ts";
import { ARTIFACT_KIND_ORDER } from "./constants.ts";

export type ArtifactCandidate = {
  kind: ArtifactKind;
  plugin: string;
  localId: string;
};

export function candidateKey(candidate: ArtifactCandidate): string {
  return `${candidate.kind}\0${candidate.plugin}\0${candidate.localId}`;
}

function slug(value: string): string {
  const result = value
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");
  if (!result) throw new Error(`surface id has no ASCII letters or digits: ${value}`);
  return result;
}

function compare(a: ArtifactCandidate, b: ArtifactCandidate): number {
  const kindDelta = ARTIFACT_KIND_ORDER.indexOf(a.kind) - ARTIFACT_KIND_ORDER.indexOf(b.kind);
  if (kindDelta !== 0) return kindDelta;
  const aKey = candidateKey(a);
  const bKey = candidateKey(b);
  return aKey < bKey ? -1 : aKey > bKey ? 1 : 0;
}

function withHash(base: string, key: string, length: number): string {
  const hash = createHash("sha256").update(key).digest("hex").slice(0, length);
  const prefix = base.slice(0, 63 - length).replace(/-+$/g, "");
  return `${prefix}-${hash}`;
}

export function assignSurfaceIds(candidates: ArtifactCandidate[]): Map<string, string> {
  const sorted = [...candidates].sort(compare);
  const seenKeys = new Set<string>();
  const bases = new Map<string, number>();
  for (const candidate of sorted) {
    const key = candidateKey(candidate);
    if (seenKeys.has(key)) throw new Error(`duplicate surface source: ${key}`);
    seenKeys.add(key);
    const base = `${slug(candidate.plugin)}-${slug(candidate.localId)}`;
    bases.set(base, (bases.get(base) ?? 0) + 1);
  }

  const claimed = new Set<string>();
  const out = new Map<string, string>();
  for (const candidate of sorted) {
    const key = candidateKey(candidate);
    const base = `${slug(candidate.plugin)}-${slug(candidate.localId)}`;
    let id = base;
    if (base.length > 64 || (bases.get(base) ?? 0) > 1) {
      for (let length = 8; length <= 60; length += 4) {
        id = withHash(base, key, length);
        if (!claimed.has(id) && !bases.has(id)) break;
      }
    }
    if (claimed.has(id) || (id !== base && bases.has(id))) {
      throw new Error(`surface id collision: ${id}`);
    }
    claimed.add(id);
    out.set(key, id);
  }
  return out;
}
