/** The plugin settings file names (#253), without the loaders that parse them. */
export const SETTINGS_FILES = {
  bashAllowlist: "bash-allowlist.txt",
  bashDenylist: "bash-denylist.txt",
  commitPrefixes: "commit-prefixes.txt",
  mcpBlocklist: "mcp-blocklist.txt",
  protectedFiles: "protected-files.txt",
  rustUnsafeExemptions: "rust-unsafe-exemptions.txt",
  codeEditRules: "code-edit-rules.json",
} as const;
