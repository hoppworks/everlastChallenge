import { test } from "node:test";
import assert from "node:assert/strict";
import { ESLint } from "eslint";

const eslint = new ESLint();
async function messages(source) {
  const [result] = await eslint.lintText(source, { filePath: "src/App.tsx" });
  return result.messages.map((message) => message.ruleId);
}
test("unhandled and void-discarded promises fail; explicit catch passes", async () => {
  for (const source of [
    "Promise.reject(new Error('lost'));",
    "void Promise.reject(new Error('lost'));",
  ]) {
    assert.ok(
      (await messages(source)).includes(
        "@typescript-eslint/no-floating-promises",
      ),
    );
  }
  assert.deepEqual(
    await messages(
      "Promise.reject(new Error('handled')).catch(console.error);",
    ),
    [],
  );
});
test("async callbacks cannot silently enter void event interfaces", async () => {
  assert.ok(
    (await messages("const run: () => void = async () => {}; run();")).includes(
      "@typescript-eslint/no-misused-promises",
    ),
  );
});
test("explicit any cannot bypass a checked interface", async () => {
  assert.ok(
    (await messages("export const bad: any = 1;")).includes(
      "@typescript-eslint/no-explicit-any",
    ),
  );
});
