import assert from "node:assert/strict";
import test from "node:test";
import {
  assessContinuity,
  parseSignature,
  readSignature,
} from "./verify-signing-continuity.mjs";

const first = `Executable=/tmp/first\nIdentifier=de.hoppworks.everlast-dictation\nCodeDirectory flags=0x10000(runtime)\nCDHash=${"a".repeat(40)}\nAuthority=Everlast Local Development\ndesignated => identifier "de.hoppworks.everlast-dictation" and anchor H"${"c".repeat(40)}"\n`;
const verified = (text) => ({ ...parseSignature(text), verified: true });
const second = first
  .replace("first", "second")
  .replace("a".repeat(40), "b".repeat(40));

test("different code under an identical verified requirement is only eligible for a live permission retest", () => {
  const result = assessContinuity(verified(first), verified(second));
  assert.equal(result.eligibleForPermissionRetest, true);
  assert.equal(result.permissionsPreserved, null);
  assert.equal(result.requiresLivePermissionRetest, true);
});

test("a changed certificate anchor cannot pass continuity", () => {
  const result = assessContinuity(
    verified(first),
    verified(second.replace("c".repeat(40), "d".repeat(40))),
  );
  assert.equal(result.sameDesignatedRequirement, false);
  assert.equal(result.eligibleForPermissionRetest, false);
});

test("ad hoc, missing, unverified or identical code cannot masquerade as an update proof", () => {
  const a = verified(first),
    b = verified(second);
  for (const candidate of [
    { ...b, adHoc: true },
    { ...b, designatedRequirement: "" },
    { ...b, verified: false },
    a,
  ]) {
    assert.equal(
      assessContinuity(a, candidate).eligibleForPermissionRetest,
      false,
    );
  }
  assert.throws(() => parseSignature("Identifier=only"), /incomplete/i);
});

test("metadata parsing alone does not claim cryptographic verification", () => {
  assert.equal(parseSignature(first).verified, false);
});

test("option-shaped input is rejected as a bundle path before invoking codesign", () => {
  assert.throws(
    () => readSignature("--file-list=everlast-signing-test-output"),
    /Expected an existing/,
  );
});
