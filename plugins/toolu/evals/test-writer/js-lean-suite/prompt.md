---
description: Few, distinct, behavior-level tests for correct code — no mocks, no implementation details, few runs.
tags: [behavior, javascript]
max_turns: 20
timeout_seconds: 600
allowed_tools: [Read, Glob, Grep, Skill, Agent, TodoWrite]
---

Save this module as `cart.mjs` and write tests for it in `cart.test.mjs` with the built-in
`node:test` and `node:assert/strict` modules. Run them with `node --test`.

Spec:

- `addItem(cart, item)` returns a new cart and never mutates its input.
- Adding an item whose `sku` is already in the cart increases that line's `qty`.
- `qty` must be a positive integer; anything else throws `RangeError`.
- `total(cart)` returns the sum of `price × qty` in cents.

```js
export function addItem(cart, item) {
  if (!Number.isInteger(item.qty) || item.qty <= 0) {
    throw new RangeError("qty must be a positive integer");
  }
  const found = cart.items.some((line) => line.sku === item.sku);
  const items = found
    ? cart.items.map((line) =>
        line.sku === item.sku ? { ...line, qty: line.qty + item.qty } : line,
      )
    : [...cart.items, { ...item }];
  return { ...cart, items };
}

export function total(cart) {
  return cart.items.reduce((sum, line) => sum + line.price * line.qty, 0);
}
```
