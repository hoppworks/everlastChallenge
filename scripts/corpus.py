#!/usr/bin/env python3
"""Freeze a partial, unqualified FLEURS corpus before any ASR comparison."""
from array import array
import hashlib
import math
import os
import sys
from contextlib import contextmanager, ExitStack
import re
import struct
from pathlib import Path

REVISION = "70bb2e84b976b7e960aa89f1c648e09c59f894dd"
RATE = 16000
MAX_FRAMES = 300 * RATE
MAX_MEMBER_BYTES = 20 * 1024 * 1024
MAX_TSV_BYTES = 2 * 1024 * 1024
ROOT = Path(__file__).resolve().parents[1]
BASE_COUNTS = {"de": 40, "en": 30}
CALIBRATION_COUNT = 10
QUIET_COUNT = 5
CENTRAL_SECONDS = (5, 10, 15)
NOISE_SEED = 0x45564552
NOISE_GAIN = 0.04

# Publisher LFS OIDs and independently hashed TSVs, pinned in tracked code.
# The mutable ignored source/archive-manifest.json is never a trust input.
PINS = {
    "de_de-dev": {"language": "de", "split": "dev", "archiveBytes": 227694463, "archiveSha256": "72cac13bbddbddf674d8102a9e7b90a3b1db5d777ed92e88eccc5ae69cb8fc9d", "tsvBytes": 221691, "tsvSha256": "055b6ddecc9ad493d9ead86babe3ebe225148dd3c174fa6f95cb83a4f9586e18"},
    "de_de-test": {"language": "de", "split": "test", "archiveBytes": 568734559, "archiveSha256": "e86b42dfcdef749926cd92135045f87c25966c09e50d01c401adb04ee7d8628f", "tsvBytes": 559542, "tsvSha256": "82fab72c58a347345675c238f2492bb997105a4ab45d69e2a2b34ed29082fa97"},
    "en_us-dev": {"language": "en", "split": "dev", "archiveBytes": 171250900, "archiveSha256": "2658fda72f199e12676ecac9415094667a4e14e149b146e568ea00b2a2f0954c", "tsvBytes": 213065, "tsvSha256": "9d57ee7e91e9d4c92edb39f6bbea668ef8dc2a3ff96eb510d5580b2ad05d17ec"},
    "en_us-test": {"language": "en", "split": "test", "archiveBytes": 289851356, "archiveSha256": "d9c2e37b41aacd41bc283554a0a82b5476b36887049774ecb2819dcaaa55a356", "tsvBytes": 367864, "tsvSha256": "74c046239374deeb60fa63f258f907388093a32bcaa3140965f70ef05c79f7ca"},
}

class CorpusError(ValueError):
    """A deterministic error without raw reference text."""

def parse_tsv(data):
    """Read the seven literal-tab columns; quote characters have no syntax."""
    if len(data) > MAX_TSV_BYTES:
        raise CorpusError("TSV exceeds the bounded metadata size")
    try:
        lines = data.decode("utf-8").splitlines()
    except UnicodeDecodeError:
        raise CorpusError("TSV is not valid UTF-8") from None
    if not 1 <= len(lines) <= 10000:
        raise CorpusError("TSV row count is invalid")
    rows, seen = [], set()
    for line in lines:
        fields = line.split("\t")
        if len(fields) != 7:
            raise CorpusError("TSV must contain exactly seven columns")
        if not re.fullmatch(r"[0-9]{1,10}", fields[0]) or not re.fullmatch(r"[0-9]{1,7}", fields[5]):
            raise CorpusError("TSV identifier or frame count is invalid")
        identifier, frames = int(fields[0]), int(fields[5])
        if identifier > 0xffffffff or not 1 <= frames <= MAX_FRAMES:
            raise CorpusError("TSV identifier or frame count exceeds its limit")
        if not re.fullmatch(r"[0-9]{1,64}\.wav", fields[1]) or fields[1] in seen:
            raise CorpusError("TSV filename is unsafe or duplicated")
        if not fields[2] or len(fields[2].encode("utf-8")) > 256 * 1024:
            raise CorpusError("TSV raw reference is empty or too large")
        seen.add(fields[1])
        rows.append({"sentenceId": identifier, "filename": fields[1], "reference": fields[2], "frames": frames})
    return rows

def select_cases(rows, count, central=False):
    eligible = [dict(row) for row in _canonical(rows)]
    if len(eligible) < count:
        raise CorpusError("Too few distinct eligible sentences")
    selected = eligible[:count]
    if central:
        used = set()
        for seconds in CENTRAL_SECONDS:
            row = next((row for row in eligible if row["sentenceId"] not in used and (seconds - 1) * RATE <= row["frames"] <= seconds * RATE), None)
            if row is None:
                raise CorpusError("A central duration band has no eligible distinct sentence")
            used.add(row["sentenceId"])
            existing = next((item for item in selected if item["sentenceId"] == row["sentenceId"]), None)
            if existing is None:
                existing = dict(row)
                selected.append(existing)
            existing["targetFrames"] = seconds * RATE
    return selected

def decode_f32_wav(data, expected_frames):
    if not 1 <= expected_frames <= MAX_FRAMES or len(data) > MAX_MEMBER_BYTES or len(data) < 12:
        raise CorpusError("Source WAV size or frame bound is invalid")
    if data[:4] != b"RIFF" or data[8:12] != b"WAVE" or struct.unpack_from("<I", data, 4)[0] + 8 != len(data):
        raise CorpusError("Source WAV has an invalid RIFF boundary")
    offset, fmt, payload, chunks = 12, None, None, 0
    while offset < len(data):
        chunks += 1
        if chunks > 256 or offset + 8 > len(data):
            raise CorpusError("Source WAV chunk structure exceeds its limit")
        name, size = data[offset:offset + 4], struct.unpack_from("<I", data, offset + 4)[0]
        start, end = offset + 8, offset + 8 + size
        if end + (size & 1) > len(data):
            raise CorpusError("Source WAV chunk is truncated")
        if name == b"fmt ":
            if fmt is not None or size < 16:
                raise CorpusError("Source WAV format chunk is invalid or duplicated")
            fmt = struct.unpack_from("<HHIIHH", data, start)
        elif name == b"data":
            if payload is not None:
                raise CorpusError("Source WAV data chunk is duplicated")
            payload = data[start:end]
        offset = end + (size & 1)
    if fmt != (3, 1, RATE, RATE * 4, 4, 32) or payload is None:
        raise CorpusError("Source WAV must be mono float32 at 16000 Hz")
    if len(payload) != expected_frames * 4:
        raise CorpusError("Source WAV frame count differs from TSV")
    pcm = array("h")
    for (sample,) in struct.iter_unpack("<f", payload):
        if not math.isfinite(sample):
            raise CorpusError("Source WAV contains a non-finite sample")
        pcm.append(max(-32768, min(32767, round(sample * 32768))))
    if sys.byteorder != "little":
        pcm.byteswap()
    return pcm.tobytes()

def pad_pcm(pcm, target_frames):
    if len(pcm) % 2 or not len(pcm) // 2 <= target_frames <= MAX_FRAMES:
        raise CorpusError("Padding would truncate audio or exceed 300 seconds")
    return pcm + bytes(target_frames * 2 - len(pcm))

def scale_pcm(pcm, gain):
    values = array("h", (max(-32768, min(32767, round(sample * gain))) for (sample,) in struct.iter_unpack("<h", pcm)))
    if sys.byteorder != "little":
        values.byteswap()
    return values.tobytes()

def pcm_wav(pcm):
    if len(pcm) % 2 or not 1 <= len(pcm) // 2 <= MAX_FRAMES:
        raise CorpusError("PCM16 frame count is invalid")
    return b"RIFF" + struct.pack("<I", 36 + len(pcm)) + b"WAVEfmt " + struct.pack("<IHHIIHH", 16, 1, 1, RATE, RATE * 2, 2, 16) + b"data" + struct.pack("<I", len(pcm)) + pcm

def _verify_open(source, expected_bytes, expected_sha256):
    digest, count = hashlib.sha256(), 0
    source.seek(0)
    if os.fstat(source.fileno()).st_size != expected_bytes:
        raise CorpusError("Pinned source byte count differs")
    while True:
        chunk = source.read(1024 * 1024)
        if not chunk:
            break
        count += len(chunk)
        if count > expected_bytes:
            raise CorpusError("Pinned source grew during verification")
        digest.update(chunk)
    if count != expected_bytes or digest.hexdigest() != expected_sha256:
        raise CorpusError("Pinned source SHA256 differs")
    source.seek(0)


def verify_file(path, expected_bytes, expected_sha256):
    """Check pinned size/hash; archives additionally retain their verified FD."""
    with open(path, "rb") as source:
        _verify_open(source, expected_bytes, expected_sha256)


class _VerifiedArchive:
    def __init__(self, path, source, expected_bytes, expected_sha256):
        self.path, self.file = Path(path), source
        self.expected_bytes, self.expected_sha256 = expected_bytes, expected_sha256
        metadata = os.fstat(source.fileno())
        self.identity = (metadata.st_dev, metadata.st_ino)


@contextmanager
def open_verified(path, expected_bytes, expected_sha256):
    with open(path, "rb") as source:
        _verify_open(source, expected_bytes, expected_sha256)
        yield _VerifiedArchive(path, source, expected_bytes, expected_sha256)


class _HashedReader:
    """Hash the exact compressed byte stream consumed by the tar decoder."""
    def __init__(self, source, expected_bytes):
        self.source, self.limit = source, expected_bytes
        self.digest, self.count = hashlib.sha256(), 0

    def read(self, size):
        data = self.source.read(min(size, 1024 * 1024) if size >= 0 else 1024 * 1024)
        self.count += len(data)
        if self.count > self.limit:
            raise CorpusError("Archive grew during extraction")
        self.digest.update(data)
        return data


def read_selected(verified, split, filenames):
    """Stream selected regular members only; never extract tar paths to disk."""
    import posixpath
    import tarfile
    import zlib
    if not isinstance(verified, _VerifiedArchive):
        raise CorpusError("Archive extraction requires a retained verified source")
    current = os.stat(verified.path)
    if (current.st_dev, current.st_ino) != verified.identity:
        raise CorpusError("Archive path changed after verification")
    verified.file.seek(0)
    reader = _HashedReader(verified.file, verified.expected_bytes)
    if split not in ("dev", "test") or any(not re.fullmatch(r"[0-9]{1,64}\.wav", name) for name in filenames):
        raise CorpusError("Selected archive basename or split is invalid")
    selected = {}
    try:
        with tarfile.open(fileobj=reader, mode="r|gz") as archive:
            for index, member in enumerate(archive):
                if index >= 10000:
                    raise CorpusError("Archive entry count exceeds its limit")
                basename = posixpath.basename(member.name)
                if basename not in filenames:
                    continue
                if member.name != split + "/" + basename:
                    raise CorpusError("Selected archive member has an unexpected path")
                if basename in selected or not member.isfile() or member.issparse():
                    raise CorpusError("Selected archive member is duplicated or not regular")
                if not 0 < member.size <= MAX_MEMBER_BYTES:
                    raise CorpusError("Selected archive member exceeds its byte limit")
                source = archive.extractfile(member)
                if source is None:
                    raise CorpusError("Selected archive member is unreadable")
                with source:
                    data = source.read(MAX_MEMBER_BYTES + 1)
                if len(data) != member.size:
                    raise CorpusError("Selected archive member is truncated")
                selected[basename] = data
        # tar stops at its end marker; pin the remaining gzip/footer/trailing
        # bytes too. No selected data escapes before this exact-stream check.
        while reader.read(1024 * 1024):
            pass
    except (tarfile.TarError, EOFError, OSError, zlib.error):
        raise CorpusError("Source archive is malformed or unreadable") from None
    if reader.count != verified.expected_bytes or reader.digest.hexdigest() != verified.expected_sha256:
        raise CorpusError("Archive bytes changed after verification")
    if set(selected) != set(filenames):
        raise CorpusError("Source archive lacks a selected basename")
    return selected

def _noise(seed):
    """Specified xorshift32-v1 stream, independent of Python's random module."""
    state = seed
    while True:
        state ^= (state << 13) & 0xffffffff
        state ^= state >> 17
        state ^= (state << 5) & 0xffffffff
        state &= 0xffffffff
        yield (state >> 16) - 32768

def generated_pcm(kind, frames=5 * RATE):
    if kind == "silence":
        return bytes(frames * 2)
    if kind != "noise":
        raise CorpusError("Unknown synthetic audio kind")
    stream = _noise(NOISE_SEED)
    values = array("h", (round(next(stream) * NOISE_GAIN) for _ in range(frames)))
    if sys.byteorder != "little":
        values.byteswap()
    return values.tobytes()

def construct_masked(clips, target_frames=MAX_FRAMES):
    """Whole PCM clips ×0.8 plus fixed ±0.03 noise; never trim or retune."""
    import itertools
    source = b"".join(clips)
    source_frames = len(source) // 2
    if not source_frames or len(source) % 2 or source_frames > target_frames or target_frames > MAX_FRAMES or target_frames % 320:
        raise CorpusError("Constructed stress audio has invalid whole-clip bounds")
    samples = itertools.chain((item[0] for item in struct.iter_unpack("<h", source)), itertools.repeat(0, target_frames - source_frames))
    noise, output, energy, minimum = _noise(0x45563300), array("h"), 0, None
    for index, sample in enumerate(samples, 1):
        mixed = round(sample * 0.8 + next(noise) * 0.03)
        if not -32768 <= mixed <= 32767:
            raise CorpusError("Constructed stress audio would clip")
        output.append(mixed)
        energy += mixed * mixed
        if index % 320 == 0:
            rms = math.sqrt(energy / 320) / 32768
            if rms < 0.005:
                raise CorpusError("Constructed stress audio contains a safe-energy-pause window")
            minimum = rms if minimum is None else min(minimum, rms)
            energy = 0
    if sys.byteorder != "little":
        output.byteswap()
    return output.tobytes(), minimum


def _publish(path, data):
    """Atomic create, never replace a frozen file; verify identical reruns."""
    import os
    import tempfile
    import stat
    path = Path(path)
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.parent.is_symlink():
        raise CorpusError("Output directory must not be redirected")
    def existing():
        metadata = path.lstat()
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size != len(data):
            raise CorpusError("Frozen output is not an identical regular file")
        with path.open("rb") as source:
            if source.read(len(data) + 1) != data:
                raise CorpusError("Frozen output bytes differ; choose a new manifest/output set")
    if path.exists() or path.is_symlink():
        existing()
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".corpus-", delete=False) as target:
            temporary = target.name
            os.fchmod(target.fileno(), 0o600)
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            existing()
    finally:
        if temporary is not None:
            os.unlink(temporary)


def _canonical(rows):
    eligible, seen = [], set()
    for row in sorted(rows, key=lambda row: (row["sentenceId"], row["filename"])):
        if 4 * RATE <= row["frames"] <= 30 * RATE and row["sentenceId"] not in seen:
            seen.add(row["sentenceId"])
            eligible.append(row)
    return eligible


def build(source_directory, manifest_path):
    with ExitStack() as retained_sources:
        return _build_retained(source_directory, manifest_path, retained_sources)


def _build_retained(source_directory, manifest_path, retained_sources):
    import json
    import unicodedata
    if unicodedata.unidata_version != "13.0.0":
        raise CorpusError("Builder requires the frozen Unicode 13.0.0 runtime")
    source_directory, manifest_path = Path(source_directory), Path(manifest_path)
    selections, rows_by_key, sources, archives = {}, {}, [], {}
    # Verify every archive and TSV before interpreting or publishing selected audio.
    for key, pin in sorted(PINS.items()):
        archive, tsv = source_directory / (key + ".tar.gz"), source_directory / (key + ".tsv")
        archives[key] = retained_sources.enter_context(open_verified(archive, pin["archiveBytes"], pin["archiveSha256"]))
        verify_file(tsv, pin["tsvBytes"], pin["tsvSha256"])
        with tsv.open("rb") as metadata:
            data = metadata.read(MAX_TSV_BYTES + 1)
        if len(data) != pin["tsvBytes"] or hashlib.sha256(data).hexdigest() != pin["tsvSha256"]:
            raise CorpusError("TSV changed after source verification")
        rows = parse_tsv(data)
        rows_by_key[key] = rows
        count = CALIBRATION_COUNT if pin["split"] == "dev" else BASE_COUNTS[pin["language"]]
        selections[key] = select_cases(rows, count, central=pin["split"] == "test")
        sources.append({"key": key, "language": pin["language"], "split": pin["split"], "archive": {"filename": archive.name, "bytes": pin["archiveBytes"], "sha256": pin["archiveSha256"]}, "tsv": {"filename": tsv.name, "bytes": pin["tsvBytes"], "sha256": pin["tsvSha256"]}})
    for language in ("de", "en"):
        ids = {}
        for key, selected in selections.items():
            if PINS[key]["language"] == language:
                ids[PINS[key]["split"]] = {row["sentenceId"] for row in selected}
        if ids["dev"] & ids["test"]:
            raise CorpusError("Calibration and acceptance share a selected sentence identifier")
    stress_rows, stress_frames = [], 0
    for row in _canonical(rows_by_key["de_de-test"]):
        if stress_frames + row["frames"] > MAX_FRAMES:
            break
        stress_rows.append(row)
        stress_frames += row["frames"]
    if not stress_rows:
        raise CorpusError("No complete natural clip fits the constructed stress case")
    decoded = {}
    for key, selected in selections.items():
        required = {row["filename"]: row for row in selected}
        if key == "de_de-test":
            required.update({row["filename"]: row for row in stress_rows})
        payloads = read_selected(archives[key], PINS[key]["split"], set(required))
        for filename, raw in payloads.items():
            decoded[(key, filename)] = (decode_f32_wav(raw, required[filename]["frames"]), hashlib.sha256(raw).hexdigest())
    cases = []
    for key, selected in selections.items():
        pin, base = PINS[key], []
        count = CALIBRATION_COUNT if pin["split"] == "dev" else BASE_COUNTS[pin["language"]]
        for row in selected:
            pcm, source_sha = decoded[(key, row["filename"])]
            changes = [{"operation": "float32_to_pcm16", "sampleRate": RATE, "rounding": "nearest-even", "scale": 32768, "saturation": [-32768, 32767]}]
            tags = ["fleurs", "read-speech"]
            if "targetFrames" in row:
                changes.append({"operation": "append_zero_frames", "frames": row["targetFrames"] - row["frames"]})
                pcm = pad_pcm(pcm, row["targetFrames"])
                tags += ["central", "central-{}s".format(row["targetFrames"] // RATE)]
            case = {"id": "fleurs-{}-{}-{}".format(pin["language"], pin["split"], row["sentenceId"]), "language": pin["language"], "class": "clean", "split": "calibration" if pin["split"] == "dev" else "acceptance", "reference": row["reference"], "tags": tags, "source": {"dataset": "google/fleurs", "revision": REVISION, "split": pin["split"], "sentenceId": row["sentenceId"], "filename": row["filename"], "sha256": source_sha, "transformations": changes}, "_pcm": pcm}
            base.append(case)
            cases.append(case)
        if count < QUIET_COUNT:
            raise CorpusError("Too few base cases for the fixed quiet selection")
        for original in base[:QUIET_COUNT]:
            quiet = dict(original)
            quiet["id"] += "-quiet"
            quiet["class"] = "challenging"
            quiet["tags"] = [tag for tag in original["tags"] if not tag.startswith("central")] + ["quiet"]
            quiet["source"] = dict(original["source"])
            quiet["source"]["transformations"] = original["source"]["transformations"] + [{"operation": "pcm16_gain", "gain": 0.05, "rounding": "nearest-even"}]
            quiet["_pcm"] = scale_pcm(original["_pcm"], 0.05)
            cases.append(quiet)
    for language in ("de", "en"):
        for kind in ("silence", "noise"):
            cases.append({"id": "synthetic-{}-{}-5s".format(language, kind), "language": language, "class": "challenging", "split": "acceptance", "reference": "", "tags": ["synthetic", kind, "not-room-noise"], "source": {"dataset": "synthetic", "revision": "corpus-builder-v1", "split": "acceptance", "sentenceId": None, "filename": None, "sha256": None, "transformations": [{"operation": "generate_" + kind, "frames": 5 * RATE, "generator": "xorshift32-v1" if kind == "noise" else "zero", "seed": NOISE_SEED if kind == "noise" else None, "uniformAmplitude": NOISE_GAIN if kind == "noise" else 0}]}, "_pcm": generated_pcm(kind)})
    clips, clip_sources, at = [], [], 0
    for row in stress_rows:
        pcm, sha = decoded[("de_de-test", row["filename"])]
        clips.append(pcm)
        clip_sources.append({"sentenceId": row["sentenceId"], "filename": row["filename"], "sha256": sha, "sourceStartFrame": 0, "sourceEndFrame": row["frames"], "outputStartFrame": at, "outputEndFrame": at + row["frames"]})
        at += row["frames"]
    stress, minimum = construct_masked(clips, MAX_FRAMES)
    cases.append({"id": "constructed-de-masked-300s", "language": "de", "class": "challenging", "split": "acceptance", "reference": " ".join(row["reference"] for row in stress_rows), "tags": ["constructed", "natural-source", "masked-pauses", "max-duration"], "source": {"dataset": "google/fleurs+synthetic-noise", "revision": REVISION, "split": "test", "sentenceId": None, "filename": None, "sha256": None, "clips": clip_sources, "transformations": [{"operation": "whole_clip_concat_masked_pauses", "speechGain": 0.8, "noiseUniformAmplitude": 0.03, "noiseSeed": 0x45563300, "generator": "xorshift32-v1", "paddingStartFrame": at, "paddingEndFrame": MAX_FRAMES, "outputFrames": MAX_FRAMES, "rmsWindowFrames": 320, "requiredMinimumWindowRms": 0.005, "measuredMinimumWindowRms": minimum}]}, "_pcm": stress})
    pending = []
    for case in cases:
        pcm = case.pop("_pcm")
        wav = pcm_wav(pcm)
        relative = "artifacts/corpus/audio/" + case["id"] + ".wav"
        case["audio"] = {"path": relative, "sha256": hashlib.sha256(wav).hexdigest(), "frames": len(pcm) // 2, "sampleRate": RATE}
        pending.append((ROOT / relative, wav))
    report = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "dataset": "google/fleurs", "revision": REVISION, "sources": sources, "selectionRules": {"sort": ["integer sentenceId", "filename"], "deduplicate": "first filename per sentenceId after 4..30-second range filter", "baseAcceptance": BASE_COUNTS, "calibrationPerLanguage": CALIBRATION_COUNT, "centralSecondsPerLanguage": list(CENTRAL_SECONDS), "centralBand": "target-1 <= source seconds <= target; first unused canonical ID; zero-pad only", "quietPerLanguageAndSplit": QUIET_COUNT, "quietGain": 0.05, "syntheticNoiseAmplitude": NOISE_GAIN, "syntheticNoiseSeed": NOISE_SEED, "stressSelection": "complete canonical DE test prefix until the next whole clip would exceed the maximum; no cuts", "maxFrames": MAX_FRAMES, "reusedReferencesAreNotIndependentCoverage": True}, "requiredCoveragePending": ["natural_de_utterances_0.2_to_1s", "natural_en_utterances_0.2_to_1s", "natural_continuous_de_300s_without_safe_pause", "natural_room_noise", "independent_listening_and_anchor_annotation_numbers_names_negations", "real_microphone_hotkey_end_to_end"], "cases": cases}
    if len(cases) > 500 or len({case["id"] for case in cases}) != len(cases):
        raise CorpusError("Corpus cases exceed the bound or contain duplicate identifiers")
    frozen = (json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    if manifest_path.exists() or manifest_path.is_symlink():
        if manifest_path.is_symlink() or not manifest_path.is_file() or manifest_path.stat().st_size != len(frozen):
            raise CorpusError("Existing frozen manifest differs")
        with manifest_path.open("rb") as old:
            if old.read(len(frozen) + 1) != frozen:
                raise CorpusError("Existing frozen manifest differs")
    if manifest_path.resolve() in {path.resolve() for path, _ in pending}:
        raise CorpusError("Manifest path collides with normalized audio")
    for path, wav in pending:
        _publish(path, wav)
    _publish(manifest_path, frozen)
    return report


def main(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True)
    parser.add_argument("--manifest", required=True)
    args = parser.parse_args(argv)
    try:
        report = build(args.source, args.manifest)
        print("corpus: froze {} cases; coverage remains explicitly partial".format(len(report["cases"])))
        return 0
    except CorpusError as error:
        print("corpus: " + str(error), file=sys.stderr)
        return 2
    except OSError:
        print("corpus: source/output I/O failed", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
