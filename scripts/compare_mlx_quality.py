"""Strict MLX screening using the existing Unicode/WER oracle; no positive delta."""
import hashlib
import importlib.util
import json
from pathlib import Path

_spec = importlib.util.spec_from_file_location("quality", Path(__file__).with_name("quality.py"))
quality = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(quality)


def compare(cases, baseline, candidate, reviews=None):
    reference = quality.evaluate(cases, baseline)
    selected = quality.evaluate(cases, candidate)
    groups = []
    passed = True
    for before, after in zip(reference["groups"], selected["groups"]):
        ok = (before["complete"] and after["complete"]
              and after["refWords"] == before["refWords"]
              and after["edits"] <= before["edits"]
              and after["unwantedTextCount"] <= before["unwantedTextCount"])
        groups.append({"split": before["split"], "language": before["language"],
                       "class": before["class"], "baselineEdits": before["edits"],
                       "candidateEdits": after["edits"], "refWords": before["refWords"],
                       "passed": ok})
        passed = passed and ok
    original = {row["id"]: row for row in baseline}
    changed = {row["id"]: row for row in candidate}
    required = []
    reviews = reviews or {}
    checked = True
    for case, before, after in zip(cases, reference["cases"], selected["cases"]):
        identifier = case["id"]
        if (after["unwantedText"] is True and before["unwantedText"] is not True
                or before["hypWords"] and after["hypWords"] == 0 and before["refWords"]):
            passed = False
        if original.get(identifier) == changed.get(identifier):
            continue
        pair = {"case": case, "baseline": original.get(identifier),
                "candidate": changed.get(identifier)}
        digest = hashlib.sha256(json.dumps(pair, ensure_ascii=False, sort_keys=True,
                                separators=(",", ":"), allow_nan=False).encode("utf-8")).hexdigest()
        review = reviews.get(identifier, {})
        accepted = (review.get("pairSha256") == digest and review.get("accepted") is True
                    and isinstance(review.get("note"), str) and bool(review["note"].strip()))
        checked = checked and accepted
        required.append({"id": identifier, "pairSha256": digest, "accepted": accepted})
    return {"schemaVersion": 1, "automaticGatePassed": bool(passed),
            "positiveWerDeltaAllowed": 0, "groups": groups,
            "reviewsRequired": required, "semanticReviewComplete": checked,
            "qualified": bool(passed and checked)}


def main():
    import argparse
    import os
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest")
    parser.add_argument("baseline")
    parser.add_argument("candidate")
    parser.add_argument("--reviews")
    parser.add_argument("--output", required=True)
    args = parser.parse_args()
    manifest = quality._load_json(args.manifest)
    if manifest.get("schemaVersion") != 1 or manifest.get("unicodeVersion") != "13.0.0":
        raise ValueError("manifest schema/Unicode version mismatch")
    reviews = quality._load_json(args.reviews) if args.reviews else None
    report = compare(manifest["cases"], quality._load_json(args.baseline),
                     quality._load_json(args.candidate), reviews)
    report["inputs"] = {name: hashlib.sha256(Path(path).read_bytes()).hexdigest()
                       for name, path in (("manifest", args.manifest), ("baseline", args.baseline),
                                          ("candidate", args.candidate))}
    with os.fdopen(os.open(args.output, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600), "w") as output:
        json.dump(report, output, ensure_ascii=False, indent=2, allow_nan=False)
        output.write("\n")
    return 0 if report["qualified"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
