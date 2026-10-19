import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import path from "node:path";
import { statSync } from "node:fs";
import { fileURLToPath } from "node:url";

// Parsing metadata cannot establish cryptographic verification.
export function parseSignature(output) {
  const identifier = output.match(/^Identifier=(.+)$/m)?.[1];
  const cdHash = output
    .match(/^CDHash=([a-f0-9]{40,64})$/im)?.[1]
    ?.toLowerCase();
  if (!identifier || !cdHash) throw new Error("Incomplete codesign metadata");
  return {
    identifier,
    cdHash,
    verified: false,
    adHoc:
      /^Signature=adhoc$/m.test(output) ||
      /^CodeDirectory.*\(.*adhoc.*\)/m.test(output),
    designatedRequirement: output.match(/^designated => (.+)$/m)?.[1] ?? "",
  };
}

export function assessContinuity(before, after) {
  const sameIdentifier = before.identifier === after.identifier;
  const sameDesignatedRequirement =
    before.designatedRequirement.length > 0 &&
    before.designatedRequirement === after.designatedRequirement;
  const differentCode = before.cdHash !== after.cdHash;
  return {
    sameIdentifier,
    sameDesignatedRequirement,
    differentCode,
    eligibleForPermissionRetest:
      before.verified === true &&
      after.verified === true &&
      !before.adHoc &&
      !after.adHoc &&
      sameIdentifier &&
      sameDesignatedRequirement &&
      differentCode,
    permissionsPreserved: null,
    requiresLivePermissionRetest: true,
    requirementSha256: createHash("sha256")
      .update(before.designatedRequirement)
      .digest("hex"),
  };
}

export function readSignature(bundle) {
  bundle = path.resolve(bundle);
  if (!bundle.endsWith(".app") || !statSync(bundle).isDirectory()) {
    throw new Error("Expected an existing .app directory");
  }
  execFileSync(
    "/usr/bin/codesign",
    ["--verify", "--deep", "--strict", bundle],
    { stdio: "pipe" },
  );
  const result = spawnSync(
    "/usr/bin/codesign",
    ["-d", "-r-", "--verbose=4", bundle],
    { encoding: "utf8" },
  );
  if (result.status !== 0) throw new Error("Unable to inspect signature");
  return { ...parseSignature(result.stdout + result.stderr), verified: true };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  try {
    if (process.argv.length !== 4)
      throw new Error(
        "Usage: node scripts/verify-signing-continuity.mjs FIRST.app SECOND.app",
      );
    const result = assessContinuity(
      readSignature(process.argv[2]),
      readSignature(process.argv[3]),
    );
    console.log(JSON.stringify(result, null, 2));
    if (!result.eligibleForPermissionRetest) process.exitCode = 1;
  } catch (error) {
    console.error(error.message);
    process.exitCode = 1;
  }
}
