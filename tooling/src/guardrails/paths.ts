/** Where the guardrails data assets live: the vendored conventions tree. */
import { resolve } from "node:path";

export const ASSETS = resolve(import.meta.dir, "../../conventions/guardrails");
