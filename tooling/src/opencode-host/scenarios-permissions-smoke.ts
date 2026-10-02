/** Focused actual-host scenarios for native permissions and pre-tool advice (#339). */
import type { PretoolScenario } from "./pretool-shared.ts";
import { ADVICE_SCENARIOS } from "./scenarios-permissions-advice.ts";
import { NATIVE_SCENARIOS } from "./scenarios-permissions-native.ts";
import { REJECTION_SCENARIOS } from "./scenarios-permissions-reject.ts";

export const PERMISSIONS_SMOKE_SCENARIOS: PretoolScenario[] = [
  ...NATIVE_SCENARIOS,
  ...REJECTION_SCENARIOS,
  ...ADVICE_SCENARIOS,
];
