#!/usr/bin/env python3
"""Append eight natural Heidelberg Digits cases; never declares P05 passed.

Developer tool: requires the existing /opt/homebrew/bin/ffmpeg. Labels are fixed
from the publisher's filename classes before decoding, never from ASR output.
"""
import argparse
import copy
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import re
import struct
import subprocess
import sys
import tarfile
import tempfile

ROOT = Path(__file__).resolve().parents[1]
FFMPEG = Path("/opt/homebrew/bin/ffmpeg")
SOURCE_BYTES = 337342212
SOURCE_SHA256 = "4980128f02deba417ad3592e37166b8af603e200e298dd6108c20c4ffaa2e610"
SOURCE_URL = "https://zenkelab.org/datasets/shd/hd_audio.tar.gz"
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_MEMBER_BYTES = 2 * 1024 * 1024
MAX_MEMBERS = 12000
RATE = 16000
REFERENCES = {"de": {8: "acht", 9: "neun"}, "en": {8: "eight", 9: "nine"}}
NAME = re.compile(r"audio/lang-(german|english)_speaker-(0[0-9]|1[01])_trial-(0|[1-9][0-9]{0,4})_digit-([0-9])\.flac")


class ShortCorpusError(ValueError):
    """Bounded protocol failure without source audio or reference text."""


def sha256(data):
    return hashlib.sha256(data).hexdigest()


def verify_source_bytes(source):
    source.seek(0)
    if os.fstat(source.fileno()).st_size != SOURCE_BYTES:
        raise ShortCorpusError("HD archive byte count differs from the fixed publisher copy")
    digest, count = hashlib.sha256(), 0
    for block in iter(lambda: source.read(1024 * 1024), b""):
        count += len(block)
        if count > SOURCE_BYTES:
            raise ShortCorpusError("HD archive exceeds its fixed size")
        digest.update(block)
    if count != SOURCE_BYTES or digest.hexdigest() != SOURCE_SHA256:
        raise ShortCorpusError("HD archive SHA256 differs from the fixed publisher copy")
    source.seek(0)


@contextmanager
def verified_source(path):
    """Check one FD before parsing and after decoding, before any publication."""
    with open(path, "rb") as source:
        verify_source_bytes(source)
        yield source
        verify_source_bytes(source)


def parse_filename(name):
    match = NAME.fullmatch(name) if isinstance(name, str) else None
    if match is None:
        raise ShortCorpusError("Unsafe or unsupported HD member name")
    language, speaker, trial, digit = match.groups()
    return {"path": name, "language": "de" if language == "german" else "en", "speaker": int(speaker), "trial": int(trial), "digit": int(digit)}


def flac_info(header):
    if len(header) < 42 or header[:4] != b"fLaC" or header[4] & 127 != 0 or int.from_bytes(header[5:8], "big") != 34:
        raise ShortCorpusError("FLAC STREAMINFO is absent or truncated")
    value = int.from_bytes(header[18:26], "big")
    result = {"sampleRate": value >> 44, "channels": ((value >> 41) & 7) + 1, "bitsPerSample": ((value >> 36) & 31) + 1, "frames": value & ((1 << 36) - 1)}
    if (result["sampleRate"], result["channels"], result["bitsPerSample"]) != (48000, 1, 16) or not 1 <= result["frames"] <= 300 * 48000:
        raise ShortCorpusError("HD source format or frame count differs from the audited archive")
    return result


def inspect_archive(archive):
    rows, members, lists = [], {}, {}
    for index, member in enumerate(archive):
        if index >= MAX_MEMBERS or member.name in members:
            raise ShortCorpusError("HD member count exceeds its bound or contains duplicates")
        members[member.name] = member
        if member.name == "audio" and member.isdir():
            continue
        if not member.isfile() or not 0 < member.size <= MAX_MEMBER_BYTES:
            raise ShortCorpusError("HD member type or size is unsafe")
        if member.name in ("train_filenames.txt", "test_filenames.txt"):
            try:
                names = archive.extractfile(member).read(MAX_MEMBER_BYTES + 1).decode("utf-8").splitlines()
            except UnicodeDecodeError:
                raise ShortCorpusError("HD split list is not UTF-8") from None
            if not names or len(names) > MAX_MEMBERS or len(names) != len(set(names)):
                raise ShortCorpusError("HD split list is empty, too long, or duplicated")
            for name in names:
                parse_filename("audio/" + name)
            lists[member.name] = {"audio/" + name for name in names}
        elif member.name == "README.md":
            continue
        else:
            row = parse_filename(member.name)
            row.update(flac_info(archive.extractfile(member).read(42)))
            rows.append(row)
    if set(lists) != {"train_filenames.txt", "test_filenames.txt"}:
        raise ShortCorpusError("Official HD split lists are missing")
    train, test = lists["train_filenames.txt"], lists["test_filenames.txt"]
    if train & test or train | test != {row["path"] for row in rows}:
        raise ShortCorpusError("HD official splits overlap or do not cover the audio members")
    for row in rows:
        row["split"] = "calibration" if row["path"] in train else "acceptance"
    return rows


def select_cases(rows):
    """One 8 and one 9 per language/split, with deterministic voice diversity."""
    train_speakers = {row["speaker"] for row in rows if row["split"] == "calibration"}
    selected = []
    for language in ("de", "en"):
        for split in ("calibration", "acceptance"):
            used = set()
            for digit in (8, 9):
                candidates = [row for row in rows if row["language"] == language and row["split"] == split and row["digit"] == digit and 9600 <= row["frames"] <= 48000]
                if not candidates:
                    raise ShortCorpusError("HD lacks a required complete short digit case")
                chosen = min(candidates, key=lambda row: (split == "acceptance" and row["speaker"] in train_speakers, row["speaker"] in used, row["path"]))
                used.add(chosen["speaker"])
                selected.append(dict(chosen, reference=REFERENCES[language][digit]))
    return selected


def tool_identity():
    digest = hashlib.sha256()
    with FFMPEG.open("rb") as binary:
        for block in iter(lambda: binary.read(1024 * 1024), b""):
            digest.update(block)
    result = subprocess.run([str(FFMPEG), "-version"], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=10, check=False)
    if result.returncode or not result.stdout or len(result.stdout) > 65536:
        raise ShortCorpusError("ffmpeg version query failed")
    return {"version": result.stdout.decode("utf-8").splitlines()[0], "sha256": digest.hexdigest(), "path": str(FFMPEG)}


def normalize_flac(data, info):
    command = [str(FFMPEG), "-hide_banner", "-loglevel", "error", "-fflags", "+bitexact", "-i", "pipe:0", "-map", "0:a:0", "-map_metadata", "-1", "-ac", "1", "-ar", str(RATE), "-c:a", "pcm_s16le", "-flags:a", "+bitexact", "-fflags", "+bitexact", "-f", "s16le", "pipe:1"]
    result = subprocess.run(command, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=False)
    pcm = result.stdout
    if result.returncode or len(pcm) % 2 or not 3200 <= len(pcm) // 2 <= 16000:
        raise ShortCorpusError("ffmpeg failed or short PCM duration differs from its bounds")
    expected = (info["frames"] * RATE + info["sampleRate"] // 2) // info["sampleRate"]
    if abs(len(pcm) // 2 - expected) > 1:
        raise ShortCorpusError("Resampling changed the complete source duration")
    # Canonical finite RIFF avoids streaming size sentinels and variable metadata.
    return b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, RATE, RATE * 2, 2, 16) + b"data" + struct.pack("<I", len(pcm)) + pcm


def append_manifest(base, cases, tool):
    if not isinstance(base, dict) or type(base.get("schemaVersion")) is not int or base["schemaVersion"] != 1 or base.get("unicodeVersion") != "13.0.0" or not isinstance(base.get("cases"), list):
        raise ShortCorpusError("Base manifest schema or Unicode version is unsupported")
    if not isinstance(base.get("sources", []), list) or not isinstance(base.get("selectionRules", {}), dict) or not isinstance(base.get("requiredCoveragePending", []), list) or any(not isinstance(item, str) for item in base.get("requiredCoveragePending", [])):
        raise ShortCorpusError("Base manifest provenance or pending coverage is invalid")
    if len(base["cases"]) + len(cases) > 500:
        raise ShortCorpusError("Extended corpus exceeds 500 cases")
    if any(not isinstance(case, dict) for case in base["cases"] + cases):
        raise ShortCorpusError("Corpus cases must be objects")
    identifiers = [case.get("id") for case in base["cases"] + cases]
    if any(not isinstance(value, str) or not value for value in identifiers) or len(set(identifiers)) != len(identifiers):
        raise ShortCorpusError("Case identifiers are missing or collide with the frozen base")
    output = copy.deepcopy(base)
    output["cases"].extend(copy.deepcopy(cases))
    output.setdefault("sources", []).append({"sourceId": "heidelberg-digits-v1.0", "dataset": "Heidelberg Digits original audio", "version": "1.0", "url": SOURCE_URL, "archiveBytes": SOURCE_BYTES, "archiveSha256": SOURCE_SHA256, "license": "CC-BY-4.0", "licenseEvidence": "archive README.md: spoken digits dataset", "attribution": "Copyright 2019 Benjamin Cramer, Yannik Stradmann, Johannes Schemmel, Friedemann Zenke; doi:10.1109/TNNLS.2020.3044364", "decoder": dict(tool)})
    output.setdefault("selectionRules", {})["heidelbergDigits"] = {"digits": [8, 9], "references": REFERENCES, "durationSeconds": [0.2, 1.0], "splitLists": {"calibration": "train_filenames.txt", "acceptance": "test_filenames.txt"}, "order": "language, split, digit; prefer acceptance speakers absent from train, then unused speaker per pair, then canonical filename", "trim": False, "referenceProvenance": "publisher filename digit/language classes; fixed word mapping before ASR; no model-generated labels"}
    achieved = set()
    for language in ("de", "en"):
        if all(sum(case["language"] == language and case["split"] == split and 3200 <= case["audio"]["frames"] <= 16000 and case["audio"]["sampleRate"] == RATE for case in cases) == 2 for split in ("calibration", "acceptance")):
            achieved.add("natural_{}_utterances_0.2_to_1s".format(language))
    output["requiredCoveragePending"] = [item for item in output.get("requiredCoveragePending", []) if item not in achieved]
    if "exact_30s_duration_cases" not in output["requiredCoveragePending"]:
        output["requiredCoveragePending"].append("exact_30s_duration_cases")
    return output


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ShortCorpusError("Duplicate JSON key")
        result[key] = value
    return result


def read_base(path):
    with open(path, "rb") as source:
        data = source.read(MAX_JSON_BYTES + 1)
    if len(data) > MAX_JSON_BYTES:
        raise ShortCorpusError("Base manifest exceeds 16 MiB")
    try:
        return json.loads(data.decode("utf-8"), object_pairs_hook=_unique_object, parse_constant=lambda _: (_ for _ in ()).throw(ShortCorpusError("Non-finite JSON number")))
    except (UnicodeError, ValueError, RecursionError) as error:
        if isinstance(error, ShortCorpusError):
            raise
        raise ShortCorpusError("Base manifest is not valid bounded JSON") from None


def preflight_identical(path, data):
    if path.is_symlink():
        raise ShortCorpusError("Frozen output must not be a symlink")
    if path.exists():
        with path.open("rb") as source:
            existing = source.read(len(data) + 1)
        if existing != data:
            raise ShortCorpusError("Frozen output differs; refusing overwrite")


def write_identical(path, data):
    preflight_identical(path, data)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.parent.is_symlink():
        raise ShortCorpusError("Frozen output directory must not be redirected")
    if path.exists():
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".short-corpus-", delete=False) as output:
            temporary = output.name
            os.fchmod(output.fileno(), 0o600)
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            preflight_identical(path, data)
    finally:
        if temporary is not None:
            os.unlink(temporary)


def build(base_path, source_path, output_path, root=ROOT):
    base_path, source_path, output_path = Path(base_path), Path(source_path), Path(output_path)
    if output_path.resolve() in (base_path.resolve(), source_path.resolve()):
        raise ShortCorpusError("Output must differ from both frozen input files")
    base = read_base(base_path)
    cases, files = [], []
    with verified_source(source_path) as source, tarfile.open(fileobj=source, mode="r:gz") as archive:
        selected = select_cases(inspect_archive(archive))
        tool = tool_identity()
        for row in selected:
            original = archive.extractfile(row["path"]).read(MAX_MEMBER_BYTES + 1)
            if len(original) > MAX_MEMBER_BYTES:
                raise ShortCorpusError("Selected FLAC exceeds its bound")
            wav = normalize_flac(original, row)
            identifier = "hd-{}-{}-s{:02d}-t{}-d{}".format(row["language"], row["split"], row["speaker"], row["trial"], row["digit"])
            relative = "artifacts/corpus/audio/" + identifier + ".wav"
            cases.append({"id": identifier, "language": row["language"], "class": "clean", "split": row["split"], "reference": row["reference"], "tags": ["natural-short", "digit-anchor", "independent-class-label"], "audio": {"path": relative, "sha256": sha256(wav), "frames": (len(wav) - 44) // 2, "sampleRate": RATE}, "source": {"dataset": "Heidelberg Digits", "version": "1.0", "split": "train" if row["split"] == "calibration" else "test", "filename": row["path"], "sha256": sha256(original), "frames": row["frames"], "sampleRate": row["sampleRate"], "channels": row["channels"], "bitsPerSample": row["bitsPerSample"], "speaker": row["speaker"], "digit": row["digit"], "referenceProvenance": "filename digit/language class mapped to fixed word before inference", "transformations": [{"operation": "full-file decode and resample", "decoder": tool, "sampleRate": RATE, "channels": 1, "format": "PCM16", "bitexact": True, "mapMetadata": -1, "trim": False, "gainChange": False}]}})
            files.append((Path(root) / relative, wav))
    extended = append_manifest(base, cases, tool)
    data = (json.dumps(extended, ensure_ascii=False, allow_nan=False, indent=2) + "\n").encode("utf-8")
    if len(data) > MAX_JSON_BYTES:
        raise ShortCorpusError("Extended manifest exceeds 16 MiB")
    for path, content in [(output_path, data)] + files:
        preflight_identical(path, content)
    for path, content in files:
        write_identical(path, content)
    write_identical(output_path, data)
    return len(cases)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("base_manifest")
    parser.add_argument("--source", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    try:
        count = build(args.base_manifest, args.source, args.output)
        print("short-corpus: appended {} fixed natural digit cases; P05 remains unqualified".format(count))
        return 0
    except (ShortCorpusError, OSError, tarfile.TarError, subprocess.SubprocessError, UnicodeError) as error:
        print("short-corpus: " + (str(error) if isinstance(error, ShortCorpusError) else "local input or conversion failed"), file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
