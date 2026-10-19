import path from "node:path";
import { fileURLToPath } from "node:url";
import { createHash } from "node:crypto";
import { constants, openSync, fstatSync, readSync, closeSync } from "node:fs";
import { awaitRecipientDelivery } from "./recipient-observation.mjs";
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
function ownedBytes(filename) {
  const fd = openSync(
    filename,
    constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK,
  );
  try {
    const stat = fstatSync(fd);
    if (!stat.isFile() || stat.uid !== process.getuid() || stat.size > 65536)
      throw new Error("Invalid owned observation file");
    const bytes = Buffer.alloc(65537);
    let size = 0;
    while (size < bytes.length) {
      const read = readSync(fd, bytes, size, bytes.length - size, null);
      if (read === 0) break;
      size += read;
    }
    if (size > 65536) throw new Error("Observation file exceeds limit");
    return bytes.subarray(0, size);
  } finally {
    closeSync(fd);
  }
}
export async function observeFiles(reportPath, attemptPath) {
  if (!path.isAbsolute(reportPath) || !path.isAbsolute(attemptPath))
    throw new Error("Absolute report and attempt paths required");
  const attemptBytes = ownedBytes(attemptPath);
  const attempt = JSON.parse(attemptBytes);
  if (
    attempt.recordingStarted !== true ||
    attempt.terminal?.phase !== "Idle" ||
    attempt.terminal.outcome !== "sent" ||
    attempt.terminal.publicSentenceMatches !== true
  )
    throw new Error(
      "Controlled completion required before recipient observation",
    );
  const boundary = {
    recipientPid: attempt.recipientPid,
    afterSequence: attempt.recipient?.sequence,
    terminalObservedUnixMillis: Date.parse(attempt.endedAtUtc),
  };
  let reportBytes;
  const result = await awaitRecipientDelivery({
    ...boundary,
    read: () => {
      reportBytes = ownedBytes(reportPath);
      return JSON.parse(reportBytes);
    },
  });
  return {
    ...result,
    ...boundary,
    attemptSha256: digest(attemptBytes),
    reportSha256: digest(reportBytes),
    helperSha256: digest(
      ownedBytes(
        fileURLToPath(new URL("./recipient-observation.mjs", import.meta.url)),
      ),
    ),
    entryPointSha256: digest(ownedBytes(fileURLToPath(import.meta.url))),
    scope:
      "Diagnostic readback only; does not start capture, press keys or approve a target",
  };
}
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 4)
      throw new Error(
        "Usage: node scripts/recipient-delivery.mjs ABS_REPORT ABS_ATTEMPT",
      );
    const result = await observeFiles(...process.argv.slice(2));
    console.log(JSON.stringify(result));
    process.exitCode = result.confirmed ? 0 : 1;
  } catch {
    console.error(
      "Recipient delivery readback failed; inspect owned report and attempt boundaries",
    );
    process.exitCode = 1;
  }
}
