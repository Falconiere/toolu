/** Thin compatibility surface — proves workspace import of @toolu/core. */
import { parseClassification } from "@toolu/core/policy";

/** Echo a classification through core to prove the workspace link. */
export function classifyStub(token: string): string {
  return parseClassification(token);
}
