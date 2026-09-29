/**
 * Per-model token pricing for the live tier's cost math — benchmarks' single
 * source of truth (originally the removed `stats` plugin's report).
 *
 * Sticker prices in $/Mtok; cost is an ESTIMATE, not an Anthropic bill.
 * Fable/Mythos $10/$50, Opus $5/$25, Haiku $1/$5, Sonnet $3/$15. An unmatched
 * model is priced at the Sonnet rate AND flagged `unknown` so the caller can warn
 * instead of silently mispricing. cache_read bills 0.1x input; cache writes
 * 1.25x (5-minute TTL) / 2x (1-hour TTL) split via
 * usage.cache_creation.ephemeral_{5m,1h}_input_tokens, falling back to
 * cache_creation_input_tokens at 1.25x when the split is absent.
 *
 * PRICING_ID is the cache fingerprint: bump it on ANY rate change. Prices as
 * of 2026-06-15.
 */
import { get } from "../../json-path.ts";

export const PRICING_ID = "2026-06-15";

type Rates = { i: number; o: number; unknown?: true };

export function rates(model: string): Rates {
  if (/fable|mythos/.test(model)) return { i: 10, o: 50 };
  if (/opus/.test(model)) return { i: 5, o: 25 };
  if (/haiku/.test(model)) return { i: 1, o: 5 };
  if (/sonnet/.test(model)) return { i: 3, o: 15 };
  return { i: 3, o: 15, unknown: true };
}

/** A usage field, 0 when absent (jq's `// 0`). */
export function field(usage: unknown, ...keys: string[]): number {
  const value = get(usage, ...keys);
  return typeof value === "number" ? value : 0;
}

/** One message's cost in dollars. */
export function messageCost(usage: unknown, r: Rates): number {
  const w5 = field(usage, "cache_creation", "ephemeral_5m_input_tokens");
  const w1 = field(usage, "cache_creation", "ephemeral_1h_input_tokens");
  const writes =
    w5 + w1 > 0
      ? w5 * 1.25 * r.i + w1 * 2 * r.i
      : field(usage, "cache_creation_input_tokens") * 1.25 * r.i;
  return (
    (field(usage, "input_tokens") * r.i +
      field(usage, "output_tokens") * r.o +
      field(usage, "cache_read_input_tokens") * r.i * 0.1 +
      writes) /
    1_000_000
  );
}
