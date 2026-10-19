import { readdirSync } from "node:fs";

export function buildPlan(profile = "regression") {
  if (!["regression", "full"].includes(profile))
    throw new Error("Unknown test profile; use regression or full");
  const plan = [
    {
      id: "rust-contracts",
      resource: "cargo",
      command: "cargo",
      args: [
        "test",
        "--workspace",
        "--lib",
        "--bins",
        "--locked",
        "--features",
        "everlast-dictation/measurement,everlast-dictation/test-control",
      ],
      requireRustTests: true,
    },
    {
      id: "ui-contracts",
      command: process.execPath,
      args: ["node_modules/vitest/vitest.mjs", "run", "--maxWorkers=2"],
    },
    {
      id: "tooling-contracts",
      command: process.execPath,
      args: [
        "--test",
        ...readdirSync(new URL(".", import.meta.url))
          .filter((name) => name.endsWith(".test.mjs"))
          .sort()
          .map((name) => `scripts/${name}`),
      ],
    },
  ];
  if (profile === "full")
    plan.push(
      {
        id: "static-checks",
        resource: "cargo",
        command: "npm",
        args: ["run", "check"],
      },
      { id: "quality-tooling", command: "npm", args: ["run", "test:quality"] },
      {
        id: "core-integration-contracts",
        resource: "cargo",
        command: "cargo",
        args: ["test", "-p", "dictation-core", "--test", "*", "--locked"],
        requireRustTests: true,
      },
      {
        id: "native-offscreen-contracts",
        resource: "cargo",
        command: "cargo",
        args: [
          "test",
          "-p",
          "macos-integration",
          "--test",
          "glass_contract",
          "--test",
          "glass_header_contract",
          "--test",
          "termination_contract",
          "--test",
          "native_workspace_contract",
          "--test",
          "native_preferences_contract",
          "--test",
          "native_field_evidence_contract",
          "--example",
          "native_text_recipient",
          "--locked",
        ],
        requireRustTests: true,
      },
    );
  return plan;
}
