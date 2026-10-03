/** Routing failures never silently become default-host launches. */
import { readFileSync } from "node:fs";
import { z } from "../hooks/dist/epic-runtime.js";
import { HOST_KINDS } from "./hosts.ts";

const LaunchRouteSchema = z.object({
  host: z.enum(HOST_KINDS).nullable(),
  model: z.string().min(1).nullable().optional(),
  effort: z.string().min(1).nullable().optional(),
});

const HostPoolSchema = z
  .array(z.object({ kind: z.enum(HOST_KINDS), cap: z.number().int().positive() }))
  .min(1);

export function loadHostPool(
  path: string,
  fallback: z.infer<typeof HostPoolSchema>,
): z.infer<typeof HostPoolSchema> {
  let source: string;
  try {
    source = readFileSync(path, "utf8");
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT")
      return HostPoolSchema.parse(fallback);
    throw error;
  }
  const pool = HostPoolSchema.parse(JSON.parse(source));
  if (new Set(pool.map((entry) => entry.kind)).size !== pool.length)
    throw new Error("duplicate hosts in saved pool");
  return pool;
}

export function loadLaunchRoute(path: string): z.infer<typeof LaunchRouteSchema> | null {
  let source: string;
  try {
    source = readFileSync(path, "utf8");
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw error;
  }
  return LaunchRouteSchema.parse(JSON.parse(source));
}
