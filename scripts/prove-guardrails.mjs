import fs from "node:fs";
import path from "node:path";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { inspectSource } from "./guardrails.mjs";
import { measureRust, checkThresholds } from "./coverage.mjs";

const directory = "tests/guardrails/.generated";
fs.rmSync(directory, { recursive: true, force: true });
fs.mkdirSync(`${directory}/src`, { recursive: true });
fs.writeFileSync(
  `${directory}/Cargo.toml`,
  '[package]\nname="guardrail-fixture"\nversion="0.0.0"\nedition="2024"\n[workspace]\n',
);
const oxlintConfig = JSON.parse(fs.readFileSync(".oxlintrc.json", "utf8"));
oxlintConfig.ignorePatterns = [];
fs.writeFileSync(`${directory}/oxlint.json`, JSON.stringify(oxlintConfig));
function command(name, args, expectedSuccess) {
  const result = spawnSync(name, args, {
    encoding: "utf8",
    env: {
      ...process.env,
      CARGO_TARGET_DIR: path.resolve("target/guardrail-fixtures"),
    },
  });
  if (result.error) throw result.error;
  assert.equal(
    result.status === 0,
    expectedSuccess,
    `${name} ${args.join(" ")}\n${result.stdout}\n${result.stderr}`,
  );
}
const lintArgs = [
  "clippy",
  "--offline",
  "--manifest-path",
  `${directory}/Cargo.toml`,
  "--",
  "-D",
  "warnings",
  "-D",
  "clippy::unwrap_used",
  "-D",
  "clippy::expect_used",
  "-D",
  "clippy::panic",
  "-D",
  "clippy::allow_attributes_without_reason",
  "-D",
  "clippy::let_underscore_must_use",
];
for (const [name, source, valid] of [
  [
    "valid Result",
    "pub fn read() -> Result<(),std::io::Error> { Ok(()) }",
    true,
  ],
  [
    "unwrap",
    "pub fn read() { let value: Option<u8> = None; value.unwrap(); }",
    false,
  ],
  [
    "expect",
    'pub fn read() { let value: Option<u8> = None; value.expect("missing"); }',
    false,
  ],
  ["panic", 'pub fn read() { panic!("failure"); }', false],
  ["unhandled Result", 'pub fn read() { std::fs::read("missing"); }', false],
  ["unreasoned allow", "#[allow(dead_code)] fn hidden() {}", false],
  ["unsafe core", "#![forbid(unsafe_code)]\npub unsafe fn invalid() {}", false],
]) {
  fs.writeFileSync(`${directory}/src/lib.rs`, source);
  command("cargo", lintArgs, valid);
  console.log(`Proved Rust rule: ${name}`);
}

const typescriptFixtures = [
  ["valid", 'export const name: string = "valid";', true],
  ["type error", "export const name: string = 42;", false],
];
for (const [name, source, valid] of typescriptFixtures) {
  fs.writeFileSync(`${directory}/sample.ts`, source);
  command(
    "node_modules/.bin/tsc",
    [
      "--strict",
      "--noEmit",
      "--skipLibCheck",
      "--ignoreConfig",
      `${directory}/sample.ts`,
    ],
    valid,
  );
  console.log(`Proved TypeScript rule: ${name}`);
}
for (const [source, valid] of [
  ['export const name = "valid";', true],
  ['// @ts-ignore\nexport const name = "bad";', false],
  ['import { test } from "vitest"; test.skip("disabled",()=>{});', false],
  [
    '// oxlint-disable-next-line no-debugger -- intentionally unused\nexport const name = "bad";',
    false,
  ],
]) {
  fs.writeFileSync(`${directory}/sample.ts`, source);
  command(
    "node_modules/.bin/oxlint",
    [
      "--config",
      `${directory}/oxlint.json`,
      "--no-ignore",
      "--deny-warnings",
      "--report-unused-disable-directives-severity",
      "error",
      `${directory}/sample.ts`,
    ],
    valid,
  );
}
assert.ok(
  inspectSource("src-tauri/src/adapter.rs", "unsafe { call() }").length,
);
assert.ok(
  inspectSource(
    "src/features/dictation/view.tsx",
    'import "@tauri-apps/api/core";',
  ).length,
);

fs.writeFileSync(
  `${directory}/src/lib.rs`,
  `pub fn choose(value: bool) -> u8 {
    if value {
        1
    } else {
        2
    }
}
#[cfg(test)] mod tests {
    #[test] fn true_branch() { assert_eq!(super::choose(true),1); }
}`,
);
const incomplete = measureRust({
  manifest: `${directory}/Cargo.toml`,
  directory: "coverage/fixture-negative",
  sourceRoot: `${directory}/src`,
});
assert.ok(
  checkThresholds(incomplete, { lines: 90 }).length,
  `Negative fixture must fail: ${JSON.stringify(incomplete)}`,
);
fs.appendFileSync(
  `${directory}/src/lib.rs`,
  "\n#[test] fn false_branch() { assert_eq!(choose(false),2); }\n",
);
const complete = measureRust({
  manifest: `${directory}/Cargo.toml`,
  directory: "coverage/fixture-positive",
  sourceRoot: `${directory}/src`,
});
assert.deepEqual(checkThresholds(complete, { lines: 90 }), []);
console.log(
  "Proved instrumented Rust coverage: failing and passing thresholds",
  incomplete,
  complete,
);

fs.writeFileSync(
  `${directory}/choice.ts`,
  "export function choice(value: boolean) {\n if(value) {\n return 1;\n } else {\n return 2;\n }\n}\n",
);
fs.writeFileSync(
  `${directory}/choice.test.ts`,
  'import { test,expect } from "vitest"; import {choice} from "./choice"; test("true",()=>{expect(choice(true)).toBe(1)});',
);
fs.writeFileSync(
  `${directory}/vitest.config.mjs`,
  `export default { test: { include: ['${directory}/choice.test.ts'], coverage: { provider: 'v8', include: ['${directory}/choice.ts'], reportsDirectory:'coverage/fixture-frontend', thresholds:{lines:80,branches:80,functions:80,statements:80} } } };`,
);
command(
  "node_modules/.bin/vitest",
  ["run", "--coverage", "--config", `${directory}/vitest.config.mjs`],
  false,
);
fs.appendFileSync(
  `${directory}/choice.test.ts`,
  '\ntest("false",()=>{expect(choice(false)).toBe(2)});',
);
command(
  "node_modules/.bin/vitest",
  ["run", "--coverage", "--config", `${directory}/vitest.config.mjs`],
  true,
);
console.log(
  "Proved instrumented frontend coverage: failing and passing thresholds",
);
