/**
 * Host-contract probe (#335): a documented plugin whose initialization throws.
 * The live harness pairs it with a tool call to observe whether the pinned
 * host fails closed or keeps running tools without this plugin.
 */
import type { Plugin } from "@opencode-ai/plugin";

export const TooluInitThrowProbe: Plugin = async () => {
  throw new Error("toolu-probe: init failure");
};
