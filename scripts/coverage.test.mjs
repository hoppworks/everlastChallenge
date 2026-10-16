import { test } from "node:test";
import assert from "node:assert/strict";
import { checkThresholds } from "./coverage.mjs";

test("coverage fails below a threshold or for missing metrics", () => {
  assert.deepEqual(checkThresholds({ lines: 90 }, { lines: 90 }), []);
  assert.match(
    checkThresholds({ lines: 89.99 }, { lines: 90 }).join(),
    /below/,
  );
  assert.match(checkThresholds({}, { lines: 90 }).join(), /missing/);
});
