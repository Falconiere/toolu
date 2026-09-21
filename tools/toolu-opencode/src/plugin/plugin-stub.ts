/** OpenCode adapter stub — proves workspace import of @toolu/core (#211 fills SDK). */
import { parseClassification } from "@toolu/core/policy";

/** Echo a classification through core to prove the workspace link. */
export function classifyStub(token: string): string {
  return parseClassification(token);
}
