/** Bootstrap Ready / NotReady union (#211). */

export type ReadyResult = {
  status: "ready";
  artifacts: string[];
};

export type NotReadyResult = {
  status: "not-ready";
  reason: string;
};

export type BootstrapResult = ReadyResult | NotReadyResult;

export function ready(artifacts: string[]): ReadyResult {
  return { status: "ready", artifacts };
}

export function notReady(reason: string): NotReadyResult {
  return { status: "not-ready", reason };
}
