import { test } from "node:test";
import assert from "node:assert/strict";
import { inspectSource } from "./guardrails.mjs";

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
