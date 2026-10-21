import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { inspectSource, inspectProject } from "./guardrails.mjs";

test("project scanning includes the host build script", () => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), "everlast-rule-"));
  try {
    fs.mkdirSync(path.join(root, "src-tauri"));
    fs.writeFileSync(
      path.join(root, "src-tauri/build.rs"),
      "fn main() { unsafe { native_call() }; }",
    );
    assert.match(inspectProject(root).join(), /build.rs: unsafe outside/);
  } finally {
    fs.rmSync(root, { recursive: true, force: true });
  }
});

test("native boundary handles dynamic templates, escaped imports and computed paths", () => {
  for (const source of [
    "export const load = () => import(`@tauri-apps/api/core`);",
    'import { invoke } from "\\u0040tauri-apps/api/core";',
    "export const load = (module: string) => import(module);",
  ]) {
    assert.match(
      inspectSource("src/ui/view.ts", source).join(),
      /IPC boundary/,
    );
  }
  assert.deepEqual(
    inspectSource(
      "src/ui/view.ts",
      'export const load = () => import("./button");',
    ),
    [],
  );
});

test("frontend native imports are confined to the IPC boundary", () => {
  assert.deepEqual(
    inspectSource(
      "src/ipc/client.ts",
      "import { invoke } from '@tauri-apps/api/core';",
    ),
    [],
  );
  assert.match(
    inspectSource(
      "src/features/dictation.ts",
      "import { invoke } from '@tauri-apps/api/core';",
    ).join(),
    /IPC boundary/,
  );
});

test("unsafe Rust stays in the native crate; src-tauri is included", () => {
  assert.deepEqual(
    inspectSource(
      "crates/macos-integration/src/focus.rs",
      "unsafe { native_call() }",
    ),
    [],
  );
  for (const file of [
    "src-tauri/build.rs",
    "crates/dictation-core/src/lib.rs",
    "src-tauri/src/main.rs",
  ]) {
    assert.match(
      inspectSource(file, "unsafe { native_call() }").join(),
      /unsafe outside/,
    );
  }
});

test("production panic shortcuts are rejected in every application Rust path", () => {
  for (const source of [
    "value.unwrap()",
    'value.expect("ready")',
    'panic!("failed")',
  ]) {
    assert.match(
      inspectSource("src-tauri/src/model/load.rs", source).join(),
      /panic shortcut/,
    );
  }
  assert.deepEqual(
    inspectSource("src-tauri/src/model/load.rs", "let model = load()?;"),
    [],
  );
});

test("coverage ignores are rejected even with a reason; lint exceptions need a reason", () => {
  for (const ignore of [
    "/* v8 ignore next -- reason */",
    "/* c8 ignore start */",
    "// istanbul ignore next",
  ]) {
    assert.match(
      inspectSource("src/ipc/client.ts", ignore).join(),
      /coverage ignore/,
    );
  }
  assert.match(
    inspectSource(
      "src/ui/button.tsx",
      "// oxlint-disable-next-line react/no-danger",
    ).join(),
    /reason/,
  );
  assert.deepEqual(
    inspectSource(
      "src/ui/button.tsx",
      "// oxlint-disable-next-line react/no-danger -- reviewed trusted markup",
    ),
    [],
  );
  assert.match(
    inspectSource("src-tauri/src/main.rs", "#[allow(dead_code)]").join(),
    /reason/,
  );
  assert.deepEqual(
    inspectSource(
      "src-tauri/src/main.rs",
      '#[allow(dead_code, reason = "platform callback")]',
    ),
    [],
  );
});

test("TypeScript review escapes and silent error handling are rejected", () => {
  for (const source of [
    "try { work(); } catch {}",
    "// eslint-disable-next-line no-eval",
    "// prettier-ignore",
    "// @ts-expect-error hidden type failure",
    "const token = Math.random();",
  ]) {
    assert.notDeepEqual(inspectSource("src/example.ts", source), [], source);
  }
  assert.deepEqual(
    inspectSource(
      "src/example.ts",
      "try { work(); } catch (error) { report(error); }",
    ),
    [],
  );
});

test("focused frontend tests are rejected so a subset cannot masquerade as the suite", () => {
  for (const source of [
    'test.only("case", () => {});',
    'describe.only("group", () => {});',
    'it.skip("case", () => {});',
  ]) {
    assert.notDeepEqual(
      inspectSource("src/example.test.ts", source),
      [],
      source,
    );
  }
});
