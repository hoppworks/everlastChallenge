#!/usr/bin/env python3
"""Freeze calibration-v2 independently; never alter or evaluate acceptance cases."""
from pathlib import Path, PurePosixPath
from array import array
import copy
import hashlib
import io
import json
import os
import stat
import struct
import subprocess
import sys
import tempfile
import unicodedata
import wave

ROOT = Path(__file__).resolve().parents[1]
BASE_SHA256 = "734199a55fe70bd0cf0dbc7df2fb3ea7afc7de14ed4daf20aa3d0a5672f0c60b"
BASE_CASE_COUNT = 34
RATE = 16000
MAX_FRAMES = 300 * RATE
MAX_JSON_BYTES = 16 * 1024 * 1024
MAX_AUDIO_BYTES = 12 * 1024 * 1024
NOISE_SEED = 0x45564332
NOISE_GAIN = 0.04
FFMPEG = Path("/opt/homebrew/bin/ffmpeg")
FFMPEG_SHA256 = "c8173e9755795978bce8e104f8d7044fabe7d6d84044aba37d3648c6f4e4e1a8"
FFMPEG_VERSION = "ffmpeg version 9.0.2 Copyright (c) 2000-2026 the FFmpeg developers"
ROOM_BYTES = 10002720
ROOM_SHA256 = "0f2131bd208b604498f32f0151e98ca7440a603d1309a25879c04a232ecba047"
ROOM_URL = "https://cdn.freesound.org/previews/456/456207_1481531-hq.mp3"

class CalibrationError(ValueError):
    """Bounded protocol error that never contains reference/audio content."""

def _read(path, maximum):
    with open(path, "rb") as source:
        if not stat.S_ISREG(os.fstat(source.fileno()).st_mode):
            raise CalibrationError("Input must be a regular file")
        data = source.read(maximum + 1)
    if len(data) > maximum:
        raise CalibrationError("Input exceeds its byte limit")
    return data


def _sha(data):
    return hashlib.sha256(data).hexdigest()


def _pcm(data, frames):
    try:
        with wave.open(io.BytesIO(data), "rb") as source:
            if (source.getnchannels(), source.getsampwidth(), source.getframerate(), source.getcomptype(), source.getnframes()) != (1, 2, RATE, "NONE", frames):
                raise CalibrationError("WAV format or frame count differs from manifest")
            pcm = source.readframes(frames + 1)
    except (wave.Error, EOFError):
        raise CalibrationError("Input WAV is malformed") from None
    if len(pcm) != frames * 2:
        raise CalibrationError("Input WAV is truncated")
    return pcm


def _wav(pcm):
    if len(pcm) % 2 or not 1 <= len(pcm) // 2 <= MAX_FRAMES:
        raise CalibrationError("PCM frame count exceeds its limit")
    target = io.BytesIO()
    with wave.open(target, "wb") as writer:
        writer.setnchannels(1)
        writer.setsampwidth(2)
        writer.setframerate(RATE)
        writer.writeframes(pcm)
    return target.getvalue()


def _noise():
    state, samples = NOISE_SEED, array("h")
    for _ in range(5 * RATE):
        state ^= (state << 13) & 0xffffffff
        state ^= state >> 17
        state ^= (state << 5) & 0xffffffff
        state &= 0xffffffff
        samples.append(round(((state >> 16) - 32768) * NOISE_GAIN))
    if sys.byteorder != "little":
        samples.byteswap()
    return samples.tobytes()


def _decoder_info():
    binary = _read(FFMPEG, 128 * 1024 * 1024)
    if _sha(binary) != FFMPEG_SHA256:
        raise CalibrationError("FFmpeg binary differs from the frozen decoder")
    result = subprocess.run([str(FFMPEG), "-version"], capture_output=True, timeout=5, check=False)
    version = result.stdout.decode("utf-8", "replace").splitlines()
    if result.returncode or not version or version[0] != FFMPEG_VERSION:
        raise CalibrationError("FFmpeg version differs from the frozen decoder")
    return {"version": FFMPEG_VERSION, "sha256": FFMPEG_SHA256, "path": str(FFMPEG)}


def decode_room(data):
    """Decode only the fixed first five seconds from already-pinned RAM bytes."""
    decoder = _decoder_info()
    command = [str(FFMPEG), "-nostdin", "-hide_banner", "-loglevel", "error", "-fflags", "+bitexact", "-i", "pipe:0", "-map", "0:a:0", "-t", "5", "-vn", "-ac", "1", "-ar", str(RATE), "-map_metadata", "-1", "-flags:a", "+bitexact", "-f", "s16le", "-acodec", "pcm_s16le", "pipe:1"]
    try:
        result = subprocess.run(command, input=data, capture_output=True, timeout=30, check=False)
    except subprocess.TimeoutExpired:
        raise CalibrationError("Room decoder exceeded its time limit") from None
    if result.returncode or len(result.stdout) != 5 * RATE * 2:
        raise CalibrationError("Room decoder failed or returned an unexpected frame count")
    return result.stdout, decoder


def _existing(path, data):
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size != len(data) or _read(path, len(data)) != data:
        raise CalibrationError("Frozen output differs; choose a new output set")


def _publish(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.parent.is_symlink():
        raise CalibrationError("Output directory is redirected")
    if path.exists() or path.is_symlink():
        _existing(path, data)
        return
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(dir=path.parent, prefix=".calibration-", delete=False) as target:
            temporary = target.name
            os.fchmod(target.fileno(), 0o600)
            target.write(data)
            target.flush()
            os.fsync(target.fileno())
        try:
            os.link(temporary, path)
        except FileExistsError:
            _existing(path, data)
    finally:
        if temporary is not None:
            os.unlink(temporary)

def build(source_manifest, room_source, output):
    if unicodedata.unidata_version != "13.0.0":
        raise CalibrationError("Calibration requires Unicode 13.0.0")
    source_manifest, room_source, output = Path(source_manifest), Path(room_source), Path(output)
    raw = _read(source_manifest, MAX_JSON_BYTES)
    if _sha(raw) != BASE_SHA256:
        raise CalibrationError("Full manifest differs from the fixed base SHA256")
    try:
        base = json.loads(raw.decode("utf-8"))
    except (ValueError, UnicodeDecodeError, RecursionError):
        raise CalibrationError("Full manifest is malformed") from None
    if not isinstance(base, dict) or base.get("schemaVersion") != 1 or base.get("unicodeVersion") != "13.0.0" or not isinstance(base.get("cases"), list):
        raise CalibrationError("Full manifest protocol is unsupported")
    if any(not isinstance(case, dict) for case in base["cases"]):
        raise CalibrationError("Full manifest contains a malformed case")
    selected = [copy.deepcopy(case) for case in base["cases"] if case.get("split") == "calibration"]
    if len(selected) != BASE_CASE_COUNT:
        raise CalibrationError("Full manifest has an unexpected calibration case count")
    first, decoded, ids = {}, {}, set()
    for case in selected:
        if case.get("language") not in ("de", "en") or case.get("class") not in ("clean", "challenging") or not isinstance(case.get("id"), str) or case["id"] in ids or not isinstance(case.get("reference"), str):
            raise CalibrationError("Inherited calibration case is invalid or duplicated")
        ids.add(case["id"])
        audio = case.get("audio", {})
        relative = PurePosixPath(audio.get("path", ""))
        if relative.is_absolute() or len(relative.parts) != 4 or relative.parts[:3] != ("artifacts", "corpus", "audio") or ".." in relative.parts or relative.suffix != ".wav":
            raise CalibrationError("Inherited audio path is unsafe")
        path = ROOT / str(relative)
        try:
            path.resolve().relative_to(ROOT.resolve())
        except ValueError:
            raise CalibrationError("Inherited audio path escapes the repository") from None
        if path.is_symlink() or type(audio.get("frames")) is not int or not 1 <= audio["frames"] <= MAX_FRAMES or audio.get("sampleRate") != RATE:
            raise CalibrationError("Inherited audio metadata is invalid")
        wav = _read(path, MAX_AUDIO_BYTES)
        if _sha(wav) != audio.get("sha256"):
            raise CalibrationError("Inherited audio SHA256 differs")
        decoded[case["id"]] = _pcm(wav, audio["frames"])
        if case["class"] == "clean" and case.get("source", {}).get("dataset") == "google/fleurs":
            first.setdefault(case["language"], case)
    if set(first) != {"de", "en"}:
        raise CalibrationError("A clean calibration FLEURS sentence is missing")
    room_bytes = _read(room_source, 10 * 1024 * 1024)
    if len(room_bytes) != ROOM_BYTES or _sha(room_bytes) != ROOM_SHA256:
        raise CalibrationError("Room source differs from its fixed byte/SHA256 pin")
    room_pcm, decoder = decode_room(room_bytes)
    if len(room_pcm) != 5 * RATE * 2:
        raise CalibrationError("Room PCM frame count differs")
    cases, pending = selected, []
    def append(identifier, language, reference, tags, source, pcm):
        if identifier in ids:
            raise CalibrationError("Added case identifier collides with the base")
        ids.add(identifier)
        wav = _wav(pcm)
        relative = "artifacts/corpus/audio/" + identifier + ".wav"
        cases.append({"id": identifier, "language": language, "class": "challenging", "split": "calibration", "reference": reference, "tags": tags, "source": source, "audio": {"path": relative, "sha256": _sha(wav), "frames": len(pcm) // 2, "sampleRate": RATE}})
        pending.append((ROOT / relative, wav))
    for language, original in first.items():
        for kind, pcm in [("zero", bytes(5 * RATE * 2)), ("noise", _noise())]:
            append("calibration-{}-{}-5s".format(language, kind), language, "", ["synthetic", "not-room-noise", kind], {"dataset": "synthetic", "revision": "calibration-v2", "split": "calibration", "sha256": None, "transformations": [{"operation": "generate_" + kind, "frames": 5 * RATE, "generator": "xorshift32-v1" if kind == "noise" else "zero", "seed": NOISE_SEED if kind == "noise" else None, "uniformAmplitude": NOISE_GAIN if kind == "noise" else 0}]}, pcm)
        tail = decoded[original["id"]] + bytes(3 * RATE * 2)
        append("calibration-{}-trailing-silence".format(language), language, original["reference"], list(original.get("tags", [])) + ["trailing-silence"], {"dataset": "google/fleurs", "revision": base["revision"], "split": "dev", "baseCaseId": original["id"], "sha256": original["audio"]["sha256"], "transformations": [{"operation": "copy_verified_pcm16_and_append_zero_frames", "sourceFrames": original["audio"]["frames"], "addedFrames": 3 * RATE, "addedSeconds": 3}]}, tail)
        append("calibration-{}-room-5s".format(language), language, "", ["natural-room-tone", "publisher-no-speech-label", "listening-pending"], {"dataset": "Freesound", "revision": "456207", "split": "calibration", "title": "Empty Office Room Tone", "author": "richwise", "license": "CC0-1.0", "url": ROOM_URL, "sha256": ROOM_SHA256, "bytes": ROOM_BYTES, "sampleRate": 48000, "channels": 2, "durationSeconds": 472.052, "referenceProvenance": "publisher-described unoccupied office; independent listening pending", "transformations": [{"operation": "decode_first_five_seconds", "decoder": decoder, "input": "verified RAM bytes via stdin", "startSeconds": 0, "endSeconds": 5, "sampleRate": RATE, "channels": 1, "format": "PCM16", "bitexact": True, "gainChange": False}]}, room_pcm)
    report = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "dataset": "calibration-v2", "revision": base["revision"], "sourceManifest": {"path": "tests/fixtures/corpus/manifest.json", "sha256": BASE_SHA256, "revision": base["revision"]}, "sources": copy.deepcopy(base.get("sources", [])), "selectionRules": {"inheritedSplit": "calibration", "inheritedCases": BASE_CASE_COUNT, "addedCases": 8, "noiseSeed": NOISE_SEED, "noiseUniformAmplitude": NOISE_GAIN, "tailSelection": "first clean FLEURS case per language in full manifest order; append exactly three seconds of zeros", "roomSelection": "fixed Freesound456207 HQ MP3, decoded interval [0,5), no gain change", "acceptanceIncluded": False}, "requiredCoveragePending": copy.deepcopy(base.get("requiredCoveragePending", [])) + ["independent_roomtone_listening"], "cases": cases}
    frozen = (json.dumps(report, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")
    if len(cases) != BASE_CASE_COUNT + 8 or len(frozen) > MAX_JSON_BYTES:
        raise CalibrationError("Calibration output count or manifest bound differs")
    if output.resolve() == source_manifest.resolve() or output.resolve() == room_source.resolve() or output.resolve() in {path.resolve() for path, _ in pending}:
        raise CalibrationError("Output aliases a source or audio path")
    if output.exists() or output.is_symlink():
        _existing(output, frozen)
    for path, data in pending:
        _publish(path, data)
    _publish(output, frozen)
    return report


def main(argv=None):
    import argparse
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source-manifest", required=True)
    parser.add_argument("--room-source", required=True)
    parser.add_argument("--output", required=True)
    args = parser.parse_args(argv)
    try:
        report = build(args.source_manifest, args.room_source, args.output)
        print("calibration: froze {} cases; no acceptance cases or ASR execution".format(len(report["cases"])))
        return 0
    except CalibrationError as error:
        print("calibration: " + str(error), file=sys.stderr)
        return 2
    except (OSError, subprocess.TimeoutExpired):
        print("calibration: bounded source/decoder/output operation failed", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
