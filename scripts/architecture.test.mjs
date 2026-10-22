import { test } from "node:test";
import assert from "node:assert/strict";
import {
  inspectFrontend,
  inspectWorkspace,
  inspectNativeInputs,
} from "./architecture.mjs";
test("frontend refuses reverse dependencies, indirect native imports and cycles", () => {
  for (const graph of [
    { "src/ipc/index.ts": 'import "../App";', "src/App.tsx": "" },
    { "src/features/view.ts": 'export * from "@tauri-apps/api/core";' },
    { "src/a.ts": 'import "./b";', "src/b.ts": 'import "./a";' },
    { "src/features/view.ts": 'import "../../scripts/secret";' },
  ])
    assert.ok(inspectFrontend(graph).length);
  assert.deepEqual(
    inspectFrontend({
      "src/App.tsx": 'import "./ipc";',
      "src/ipc/index.ts": 'import "@tauri-apps/api/core";',
    }),
    [],
  );
});
test("core cannot acquire OS or inference dependencies, including build-time ones", () => {
  const metadata = (name) => ({
    workspace_members: ["core"],
    packages: [
      {
        id: "core",
        name: "dictation-core",
        dependencies: [{ name, kind: "build" }],
      },
    ],
  });
  assert.ok(inspectWorkspace(metadata("tauri")).length);
  assert.deepEqual(inspectWorkspace(metadata("serde")), []);
});
test("native edits, additions and deletions cannot bypass incremental build coverage", () => {
  assert.ok(
    inspectNativeInputs({ "x.cpp": "new" }, { "x.cpp": "old" }, []).length,
  );
  assert.ok(inspectNativeInputs({ "y.cpp": "new" }, {}, []).length);
  assert.ok(inspectNativeInputs({}, { "x.cpp": "old" }, ["x.cpp"]).length);
  assert.deepEqual(
    inspectNativeInputs({ "x.cpp": "new" }, { "x.cpp": "old" }, ["x.cpp"]),
    [],
  );
});

test("production features cannot hide controls behind aliases or cycles", () => {
  const check = (features) =>
    inspectWorkspace({
      workspace_members: ["app"],
      packages: [
        { id: "app", name: "everlast-dictation", dependencies: [], features },
      ],
    });
  assert.ok(
    check({
      default: ["first"],
      first: ["second"],
      second: ["first", "measurement"],
    }).length,
  );
  assert.ok(
    check({ default: ["native"], native: ["macos-integration/test-control"] })
      .length,
  );
  assert.deepEqual(
    check({ default: ["first"], first: ["second"], second: ["first"] }),
    [],
  );
});

test("dependency feature requests and native defaults cannot enable test controls", () => {
  for (const pkg of [
    {
      id: "app",
      name: "everlast-dictation",
      features: { default: [] },
      dependencies: [
        { name: "macos-integration", kind: null, features: ["test-control"] },
      ],
    },
    {
      id: "native",
      name: "macos-integration",
      features: { default: ["alias"], alias: ["test-control"] },
      dependencies: [],
    },
  ])
    assert.ok(
      inspectWorkspace({ workspace_members: [pkg.id], packages: [pkg] }).length,
    );
});

test("incoming dependency feature aliases are followed across crate boundaries", () => {
  const packages = [
    {
      id: "app",
      name: "everlast-dictation",
      features: { default: [] },
      dependencies: [
        { name: "macos-integration", kind: null, features: ["diagnostics"] },
      ],
    },
    {
      id: "native",
      name: "macos-integration",
      features: {
        default: [],
        diagnostics: ["again"],
        again: ["test-control"],
      },
      dependencies: [],
    },
  ];
  assert.ok(
    inspectWorkspace({ workspace_members: ["app", "native"], packages }).length,
  );
});

test("process fixtures cannot enter a production feature graph", () => {
  const worker = {
    id: "worker",
    name: "text-refinement-worker",
    features: { default: ["indirect"], indirect: ["test-fixture"] },
    dependencies: [],
  };
  assert.ok(
    inspectWorkspace({ workspace_members: [worker.id], packages: [worker] })
      .length,
  );
});
