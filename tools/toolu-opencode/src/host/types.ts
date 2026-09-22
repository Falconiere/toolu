/** Host detection types for OpenCode bootstrap (#211). */

export type TooluHost = "claude" | "codex" | "opencode";

export type HostDetectOptions = {
  env?: NodeJS.ProcessEnv;
  projectRoot?: string;
};

export type OpencodeRootsOptions = {
  env?: NodeJS.ProcessEnv;
  dataRoot?: string;
  projectRoot?: string;
};
