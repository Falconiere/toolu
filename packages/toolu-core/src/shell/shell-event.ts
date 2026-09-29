/**
 * One parse per `shell/pre` event (#284). Building the event parses nothing: the
 * first module that asks pays for `analyzeShell`, and every later module gets the
 * same object. The `WeakMap` holds no strong reference, so the analysis lives as
 * long as the event does.
 */
import type { NormalizedEvent } from "../events/events.ts";
import { analyzeShell } from "./shell-parse.ts";
import type { ShellAnalysis } from "./shell-types.ts";

export type ShellPreEvent = Extract<NormalizedEvent, { type: "shell/pre" }>;

const analyses = new WeakMap<ShellPreEvent, ShellAnalysis>();

export function shellAnalysisOf(event: ShellPreEvent): ShellAnalysis {
  const cached = analyses.get(event);
  if (cached !== undefined) return cached;
  const analysis = analyzeShell(event.command);
  analyses.set(event, analysis);
  return analysis;
}
