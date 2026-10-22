import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { validate } from "../fixtures/browser-fields/validate.ts";
const html = readFileSync(
  new URL("../fixtures/browser-fields/index.html", import.meta.url),
  "utf8",
);
const matrix: unknown = JSON.parse(
  readFileSync(
    new URL("../fixtures/browser-fields/matrix.json", import.meta.url),
    "utf8",
  ),
);
await test("public fixture contains 18 fields and 20 prospective workflows", () =>
  assert.deepEqual(validate(html, matrix), {
    htmlCases: 18,
    workflows: 20,
    nativeExecutionPerformed: false,
  }));
await test("fixture rejects network, duplicate identity, concealed readonly and secret reads", () => {
  const mutations = [
    html.replace('href="#positive"', 'href="https://example.invalid/"'),
    html.replace('id="editable-other"', 'id="editable-input"'),
    html.replace(
      'id="password-input" data-case="S01"',
      'id="password-input" value="secret" data-case="S01"',
    ),
    html.replace(
      'id="aria-input" data-case="N03"',
      'id="aria-input" readonly data-case="N03"',
    ),
    ...[
      'console.log(document.getElementById("password-input").value);',
      'fetch("https://example.invalid/");',
      'localStorage.setItem("x","y");',
    ].map((code) =>
      html.replace(
        '<script id="fixture-code">',
        '<script id="fixture-code">' + code,
      ),
    ),
  ];
  for (const changed of mutations)
    assert.throws(() => validate(changed, matrix));
});
