import path from "node:path";
import { fileURLToPath } from "node:url";
import { readRecipient } from "./autonomous-tests.mjs";

export function main(args) {
  if (args.length !== 1 || !path.isAbsolute(args[0]))
    throw new Error(
      "Usage: npm run test:os:readiness -- /absolute/snapshot.json",
    );
  const result = readRecipient(args[0]);
  console.log(JSON.stringify(result));
  return result.ready ? 0 : 1;
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = main(process.argv.slice(2));
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
