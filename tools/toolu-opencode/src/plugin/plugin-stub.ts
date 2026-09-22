/** Thin compatibility surface — core policy + host detect (#211). */
import { parseClassification } from "@toolu/core/policy";
import { detectHost } from "../host/detect.ts";

/** Echo a classification through core to prove the workspace link. */
export function classifyStub(token: string): string {
  return parseClassification(token);
}

export { detectHost };
