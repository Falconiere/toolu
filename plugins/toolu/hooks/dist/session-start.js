// @bun
// packages/toolu-core/src/launcher/launcher.ts
var ENFORCING_EVENTS = new Set(["PreToolUse", "PermissionRequest"]);
function runtimeDiagnostic(bunPath, bunVersion) {
  return { systemMessage: `toolu runtime: bun ${bunVersion} at ${bunPath}` };
}

// plugins/toolu/hooks/src/session-start.ts
process.stdout.write(`${JSON.stringify(runtimeDiagnostic(process.execPath, Bun.version))}
`);
