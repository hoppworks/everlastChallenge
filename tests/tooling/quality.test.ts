import assert from "node:assert/strict";
import { test } from "node:test";
import { evaluate, normalize } from "../../scripts/tooling/quality.ts";

await test("normalization retains the frozen Unicode 13 punctuation, casing and whitespace protocol", () => {
  assert.deepEqual(normalize("  A\u0308PFEL—İSTANBUL\tΟΣ ΟΣΑ 𐕰\u001cX "), [
    "äpfel",
    "i̇stanbul",
    "ος",
    "οσα",
    "𐕰\u001cx",
  ]);
  assert.deepEqual(normalize("a\u00a0b\u200bc"), ["a", "b\u200bc"]);
});
await test("quality distinguishes failed, missing and unwanted silence without leaking text", () => {
  const report = evaluate(
    [
      {
        id: "speech",
        language: "de",
        class: "clean",
        split: "acceptance",
        reference: "eins zwei drei",
      },
      {
        id: "missing",
        language: "de",
        class: "clean",
        split: "acceptance",
        reference: "vier",
      },
      {
        id: "silence",
        language: "de",
        class: "clean",
        split: "acceptance",
        reference: "",
      },
    ],
    [
      { id: "speech", status: "ok", text: "eins drei" },
      { id: "silence", status: "ok", text: "!" },
    ],
  );
  assert.equal(report.cases[0]?.edits, 1);
  assert.equal(report.cases[1]?.status, "missing");
  assert.equal(report.cases[2]?.unwantedText, true);
  assert.equal(report.groups[0]?.wer, null);
  assert.equal(report.groups[0]?.observedWer, 1 / 3);
  assert.ok(!JSON.stringify(report).includes("eins"));
});
await test("quality rejects unknown results, duplicate cases and unpaired surrogate input", () => {
  const c = {
    id: "a",
    language: "de",
    class: "clean",
    split: "acceptance",
    reference: "a",
  };
  assert.throws(() => evaluate([c, c], []), /Duplicate/);
  assert.throws(
    () => evaluate([c], [{ id: "b", status: "ok", text: "a" }]),
    /absent/,
  );
  assert.throws(() => normalize("\ud800"), /Unicode/);
});
await test("case enum fields reject arrays and objects instead of coercing them", () => {
  const base = {
    id: "a",
    language: "de",
    class: "clean",
    split: "acceptance",
    reference: "one",
  };
  for (const [field, value] of [
    ["language", ["de"]],
    ["class", ["clean"]],
    ["split", ["acceptance"]],
    ["language", {}],
  ] as const)
    assert.throws(() => evaluate([{ ...base, [field]: value }], []));
});
