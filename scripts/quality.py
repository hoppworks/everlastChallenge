#!/usr/bin/env python3
"""Bounded WER evaluation under the frozen Unicode 13.0.0 protocol.

Run with /usr/bin/python3 (3.9.6): quality.py MANIFEST RESULTS --output REPORT.
The manifest uses schemaVersion=1, unicodeVersion=13.0.0 and a cases array.
Exit zero means a numeric report was produced, never that P05 passed. Anchor
fidelity, names, numbers, negation and model qualification require separate review.

Bounds: 500 cases/results, 256 KiB UTF-8 per text, 16384 tokens per text,
2 million DP cells per pair and 20 million cells per evaluation. JSON input
files are each bounded at 16 MiB. Reports contain identifiers and counts only.
"""
import re
import unicodedata

UNICODE_VERSION = "13.0.0"
MAX_CASES = 500
MAX_TEXT_BYTES = 256 * 1024
MAX_TOKENS = 16384
MAX_PAIR_CELLS = 2_000_000
MAX_TOTAL_CELLS = 20_000_000

class QualityError(ValueError):
    """A bounded, source-text-free protocol error."""

# Unicode White_Space, deliberately excluding Python's extra U+001C..U+001F.
WHITE_SPACE = frozenset("\t\n\v\f\r \u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000")
SPLIT = re.compile("[" + "".join(sorted(WHITE_SPACE)) + "]+")

def _check_unicode():
    if unicodedata.unidata_version != UNICODE_VERSION:
        raise QualityError("Unicode 13.0.0 is required; use the frozen evaluator runtime")

def _text(text):
    if not isinstance(text, str):
        raise QualityError("Text must be a string")
    try:
        length = len(text.encode("utf-8"))
    except UnicodeEncodeError:
        raise QualityError("Text contains invalid Unicode") from None
    if length > MAX_TEXT_BYTES:
        raise QualityError("Text exceeds the UTF-8 byte limit")
    return text

def normalize(text):
    """NFC → Unicode lower → P* to spaces → Unicode White_Space tokens."""
    _check_unicode()
    value = unicodedata.normalize("NFC", _text(text)).lower()
    value = "".join(" " if unicodedata.category(char).startswith("P") else char for char in value)
    _text(value)
    tokens = [token for token in SPLIT.split(value) if token]
    if len(tokens) > MAX_TOKENS:
        raise QualityError("Text exceeds the token limit")
    return tokens

def _identifier(value):
    if not isinstance(value, str) or not value or value.strip() != value:
        raise QualityError("Every case/result needs a nonempty stable identifier")
    if len(_text(value).encode("utf-8")) > 256 or any(unicodedata.category(char).startswith("C") for char in value):
        raise QualityError("Identifier is invalid or too large")
    return value

def _distance(reference, hypothesis):
    if len(reference) < len(hypothesis):
        reference, hypothesis = hypothesis, reference
    previous = list(range(len(hypothesis) + 1))
    for row, word in enumerate(reference, 1):
        current = [row]
        for column, other in enumerate(hypothesis, 1):
            current.append(min(previous[column] + 1, current[-1] + 1, previous[column - 1] + (word != other)))
        previous = current
    return previous[-1]

def evaluate(cases, results):
    """Return content-free numeric scores; incomplete groups have no final WER.

    observedWer is explicitly partial when any result failed or is missing.
    Silence uses no WER denominator. Any non-White_Space raw output on silence
    is unwantedText, including punctuation removed by the WER normalizer.
    """
    _check_unicode()
    if not isinstance(cases, list) or not 1 <= len(cases) <= MAX_CASES:
        raise QualityError("Case count must be between 1 and 500")
    if not isinstance(results, list) or len(results) > MAX_CASES:
        raise QualityError("Result count exceeds its limit or is not a list")
    indexed = {}
    for case in cases:
        if not isinstance(case, dict) or set(case) - {"id", "language", "class", "split", "reference", "audio", "tags", "source"}:
            raise QualityError("Case contains unsupported fields")
        identifier = _identifier(case.get("id"))
        if identifier in indexed:
            raise QualityError("Duplicate case identifier")
        if case.get("language") not in ("de", "en") or case.get("class") not in ("clean", "challenging") or case.get("split") not in ("calibration", "acceptance"):
            raise QualityError("Case language, class or split is invalid")
        indexed[identifier] = (case, normalize(case.get("reference")))
    by_id = {}
    for result in results:
        if not isinstance(result, dict) or set(result) != {"id", "status", "text"}:
            raise QualityError("Result fields must be id, status and text")
        identifier = _identifier(result["id"])
        if identifier not in indexed:
            raise QualityError("Result identifier is absent from the manifest")
        if identifier in by_id:
            raise QualityError("Duplicate result identifier")
        if result["status"] not in ("ok", "failed") or (result["status"] == "ok" and result["text"] is None):
            raise QualityError("Result status or successful text is invalid")
        tokens = None if result["text"] is None else normalize(result["text"])
        by_id[identifier] = (result, tokens)
    total_cells = 0
    for identifier, (_, reference) in indexed.items():
        if identifier in by_id:
            result, hypothesis = by_id[identifier]
            if result["status"] == "ok" and reference:
                cells = (len(reference) + 1) * (len(hypothesis) + 1)
                total_cells += cells
                if cells > MAX_PAIR_CELLS or total_cells > MAX_TOTAL_CELLS:
                    raise QualityError("Word-distance work budget exceeded")
    scored = []
    groups = {}
    for identifier, (case, reference) in indexed.items():
        result, hypothesis = by_id.get(identifier, ({"status": "missing", "text": None}, None))
        status = result["status"]
        edits = _distance(reference, hypothesis) if status == "ok" and reference else None
        wer = edits / len(reference) if edits is not None else None
        unwanted = (any(char not in WHITE_SPACE for char in result["text"]) if result["text"] is not None else None) if not reference else False
        scored.append({
            "id": identifier,
            "split": case["split"],
            "language": case["language"],
            "class": case["class"],
            "status": status,
            "edits": edits,
            "refWords": len(reference),
            "hypWords": None if hypothesis is None else len(hypothesis),
            "wer": wer,
            "unwantedText": unwanted,
        })
        key = (case["split"], case["language"], case["class"])
        group = groups.setdefault(key, {
            "split": key[0], "language": key[1], "class": key[2],
            "caseCount": 0, "scoredCount": 0, "failedCount": 0,
            "missingCount": 0, "silenceCount": 0, "silenceFailedCount": 0,
            "unwantedTextCount": 0, "expectedRefWords": 0,
            "refWords": 0, "edits": 0,
        })
        group["caseCount"] += 1
        group["expectedRefWords"] += len(reference)
        group["failedCount"] += status != "ok"
        group["missingCount"] += status == "missing"
        group["silenceCount"] += not reference
        group["silenceFailedCount"] += not reference and status != "ok"
        group["unwantedTextCount"] += unwanted is True
        if edits is not None:
            group["scoredCount"] += 1
            group["refWords"] += len(reference)
            group["edits"] += edits
    ordered = []
    for key in sorted(groups):
        group = groups[key]
        group["complete"] = group["failedCount"] == 0
        group["observedWer"] = group["edits"] / group["refWords"] if group["refWords"] else None
        group["wer"] = group["observedWer"] if group["complete"] else None
        ordered.append(group)
    return {"schemaVersion": 1, "unicodeVersion": UNICODE_VERSION, "cases": scored, "groups": ordered}

MAX_FILE_BYTES = 16 * 1024 * 1024

def _unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise QualityError("JSON contains duplicate object keys")
        value[key] = item
    return value

def _invalid_number(_value):
    raise QualityError("JSON contains a non-finite number")

def _load_json(path):
    import json
    with open(path, "rb") as source:
        data = source.read(MAX_FILE_BYTES + 1)
    if len(data) > MAX_FILE_BYTES:
        raise QualityError("JSON file exceeds the 16 MiB limit")
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=_unique_object, parse_constant=_invalid_number)
    except (UnicodeDecodeError, ValueError, RecursionError) as error:
        if isinstance(error, QualityError):
            raise
        raise QualityError("Input is not valid UTF-8 JSON") from None

def main(argv=None):
    import argparse
    import json
    import os
    from pathlib import Path
    import sys
    import tempfile
    parser = argparse.ArgumentParser(description=__doc__, epilog="Scores alone never establish P05 acceptance or anchor fidelity.")
    parser.add_argument("manifest")
    parser.add_argument("results")
    parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    temporary = None
    try:
        _check_unicode()
        manifest = _load_json(args.manifest)
        if not isinstance(manifest, dict) or type(manifest.get("schemaVersion")) is not int or manifest["schemaVersion"] != 1:
            raise QualityError("Manifest schemaVersion must be 1")
        if manifest.get("unicodeVersion") != UNICODE_VERSION:
            raise QualityError("Manifest unicodeVersion must be 13.0.0")
        report = evaluate(manifest.get("cases"), _load_json(args.results))
        destination = Path(args.output)
        if destination.resolve() in (Path(args.manifest).resolve(), Path(args.results).resolve()):
            raise QualityError("Report output must differ from the input paths")
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=destination.parent, prefix=".quality-", suffix=".tmp", delete=False) as output:
            temporary = output.name
            json.dump(report, output, ensure_ascii=False, allow_nan=False, indent=2)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        # link is an atomic no-replace publication on the same filesystem.
        # An existing report (including a symlink) is never followed or replaced.
        try:
            os.link(temporary, destination)
        except FileExistsError:
            raise QualityError("Report output already exists; choose a new measurement path") from None
        failures = sum(case["status"] != "ok" for case in report["cases"])
        unwanted = sum(case["unwantedText"] is True for case in report["cases"])
        print("quality: evaluated {} cases in {} groups; {} failed/missing; {} unwanted silence outputs".format(len(report["cases"]), len(report["groups"]), failures, unwanted))
        return 0
    except QualityError as error:
        print("quality: " + str(error), file=sys.stderr)
        return 2
    except OSError:
        print("quality: file I/O failed", file=sys.stderr)
        return 2
    finally:
        if temporary is not None:
            try:
                os.unlink(temporary)
            except OSError:
                pass

if __name__ == "__main__":
    raise SystemExit(main())
