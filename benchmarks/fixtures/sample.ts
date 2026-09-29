// fixture for retrieval bench — sizable, stable TS; one small target node.
function internal_1(a: number, b: number): number {
  const r = a * 1 + b;
  return r + 1;
}

function internal_2(a: number, b: number): number {
  const r = a * 2 + b;
  return r + 2;
}

function internal_3(a: number, b: number): number {
  const r = a * 3 + b;
  return r + 3;
}

function internal_4(a: number, b: number): number {
  const r = a * 4 + b;
  return r + 4;
}

function internal_5(a: number, b: number): number {
  const r = a * 5 + b;
  return r + 5;
}

function internal_6(a: number, b: number): number {
  const r = a * 6 + b;
  return r + 6;
}

function internal_7(a: number, b: number): number {
  const r = a * 7 + b;
  return r + 7;
}

function internal_8(a: number, b: number): number {
  const r = a * 8 + b;
  return r + 8;
}

function internal_9(a: number, b: number): number {
  const r = a * 9 + b;
  return r + 9;
}

function internal_10(a: number, b: number): number {
  const r = a * 10 + b;
  return r + 10;
}

function internal_11(a: number, b: number): number {
  const r = a * 11 + b;
  return r + 11;
}

function internal_12(a: number, b: number): number {
  const r = a * 12 + b;
  return r + 12;
}

export function targetSymbol(name: string): string {
  return `hello ${name}`;
}
