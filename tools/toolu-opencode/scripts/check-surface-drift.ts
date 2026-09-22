/** Drift check wrapper for committed OpenCode surface (#206). */
import { runGenerateSurface } from "./generate-surface.ts";

process.exit(runGenerateSurface(["--check"]));
