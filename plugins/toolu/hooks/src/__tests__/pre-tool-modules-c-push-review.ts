/** push-review cases (#262): every `@test` of the deleted push-review.bats. */
import { featureRepo, group, type GateCase } from "./pre-tool-modules-c-cases.ts";

const pr = group({ setup: featureRepo });

export const PUSH_REVIEW_CASES: GateCase[] = [
  pr({
    name: "push-review: no state file advises at the shipped preset",
    command: "git push",
    expect: "advisory",
    has: ["Code review required before push"],
  }),
  pr({
    name: "push-review: no state file denies under strict",
    config: { version: 1, gates: { preset: "strict" } },
    command: "git push",
    expect: "deny",
    has: ["Code review required before push"],
  }),
];
