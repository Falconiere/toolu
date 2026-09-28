/**
 * SessionStart runtime diagnostic (#250): reports once per session start or
 * resume which Bun runs toolu's hooks. The launcher in hooks.json only reaches
 * this bundle when it found Bun; without Bun it prints its own advisory instead.
 * #263 folds the bash session-start.sh into this entry.
 */
import { runtimeDiagnostic } from "@toolu/core/launcher";

process.stdout.write(`${JSON.stringify(runtimeDiagnostic(process.execPath, Bun.version))}\n`);
