/** Deterministic surface id assignment (#206). */
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

export function assignSurfaceIds(
  candidates: ArtifactCandidate[],
): Map<string, string> {
  const sorted = [...candidates].sort((a, b) => {
    const kindDelta =
      ARTIFACT_KIND_ORDER.indexOf(a.kind) - ARTIFACT_KIND_ORDER.indexOf(b.kind);
    if (kindDelta !== 0) {
      return kindDelta;
    }
    const pluginDelta = a.plugin.localeCompare(b.plugin);
    if (pluginDelta !== 0) {
      return pluginDelta;
    }
    return a.localId.localeCompare(b.localId);
  });

  const claimed = new Map<string, ArtifactKind>();
  const out = new Map<string, string>();

  for (const item of sorted) {
    const key = candidateKey(item);
    const base = `${item.plugin}--${item.localId}`;
    const priorKind = claimed.get(base);
    if (!priorKind) {
      claimed.set(base, item.kind);
      out.set(key, base);
      continue;
    }
    const suffixed = `${base}--${item.kind}`;
    if (claimed.has(suffixed)) {
      throw new Error(`surface id collision: ${suffixed}`);
    }
    claimed.set(suffixed, item.kind);
    out.set(key, suffixed);
  }

  return out;
}
