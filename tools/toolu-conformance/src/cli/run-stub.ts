/** Conformance CLI stub — second real consumer of @toolu/core (#212 fills suites). */
import { parseTooluConfig } from "@toolu/core/config";

/** Validate a minimal toolu.config.json through core. */
export function checkConfigStub(input: unknown): { version: 1 } {
  return parseTooluConfig(input);
}
