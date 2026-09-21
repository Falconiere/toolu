import { expect, test } from "bun:test";
import { checkConfigStub } from "../run-stub.ts";

test("checkConfigStub accepts version 1", () => {
  expect(checkConfigStub({ version: 1 })).toEqual({ version: 1 });
});
