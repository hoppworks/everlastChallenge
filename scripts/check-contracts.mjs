import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { execFileSync } from "node:child_process";
import * as prettier from "prettier";

const directory = fs.mkdtempSync(path.join(os.tmpdir(), "everlast-contracts-"));
try {
  execFileSync(
    "cargo",
    [
      "run",
      "--quiet",
      "--locked",
      "-p",
      "everlast-dictation",
      "--example",
      "export_contracts",
      "--",
      directory,
    ],
    { stdio: "inherit" },
  );
  const expected = fs.readdirSync(directory).sort();
  const actual = fs.readdirSync("src/ipc/generated").sort();
  if (JSON.stringify(expected) !== JSON.stringify(actual))
    throw new Error("Generated IPC file set is stale");
  for (const file of expected) {
    const generated = await prettier.format(
      fs.readFileSync(path.join(directory, file), "utf8"),
      { parser: "typescript" },
    );
    if (
      generated !==
      fs.readFileSync(path.join("src/ipc/generated", file), "utf8")
    )
      throw new Error(`Generated IPC contract is stale: ${file}`);
  }
} finally {
  fs.rmSync(directory, { recursive: true, force: true });
}
