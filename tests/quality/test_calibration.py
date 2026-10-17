import importlib.util
import io
import json
import hashlib
from pathlib import Path
import struct
import tempfile
import unittest
from unittest import mock
import wave
import subprocess
from contextlib import contextmanager, redirect_stdout

SPEC = importlib.util.spec_from_file_location("calibration", Path(__file__).resolve().parents[2] / "scripts" / "calibration.py")
calibration = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(calibration)

class CalibrationTests(unittest.TestCase):
    @staticmethod
    def wav(samples):
        stream = io.BytesIO()
        with wave.open(stream, "wb") as target:
            target.setnchannels(1)
            target.setsampwidth(2)
            target.setframerate(16000)
            target.writeframes(struct.pack("<{}h".format(len(samples)), *samples))
        return stream.getvalue()

    @contextmanager
    def fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            audio = root / "artifacts/corpus/audio"
            audio.mkdir(parents=True)
            cases = []
            for language in ["en", "de"]:
                wav = self.wav([1000, -1000] * 160)
                path = audio / (language + ".wav")
                path.write_bytes(wav)
                cases.append({"id": language + "-cal", "language": language, "split": "calibration", "class": "clean", "reference": "independent " + language, "tags": ["fleurs"], "source": {"dataset": "google/fleurs", "revision": "pinned-revision", "split": "dev"}, "audio": {"path": path.relative_to(root).as_posix(), "sha256": hashlib.sha256(wav).hexdigest(), "frames": 320, "sampleRate": 16000}})
            full = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "revision": "pinned-revision", "cases": [cases[0], {"id": "acceptance-never-read", "split": "acceptance", "audio": {"path": "must-not-exist.wav"}}, cases[1]], "sources": [], "requiredCoveragePending": ["anchors"]}
            source = root / "manifest.json"
            source.write_text(json.dumps(full), encoding="utf-8")
            room = root / "room.mp3"
            room.write_bytes(b"fixed room source")
            output = root / "calibration.json"
            decoder = {"version": "fixture-decoder", "sha256": "fixture-hash"}
            with mock.patch.object(calibration, "ROOT", root), mock.patch.object(calibration, "BASE_SHA256", hashlib.sha256(source.read_bytes()).hexdigest()), mock.patch.object(calibration, "BASE_CASE_COUNT", 2), mock.patch.object(calibration, "ROOM_BYTES", room.stat().st_size), mock.patch.object(calibration, "ROOM_SHA256", hashlib.sha256(room.read_bytes()).hexdigest()), mock.patch.object(calibration, "decode_room", return_value=(bytes(160000), decoder)):
                yield root, source, room, output, cases

    def test_frozen_builder_keeps_only_original_calibration_and_adds_independent_empty_and_tail_probes(self):
        with self.fixture() as (root, source, room, output, cases):
            frozen = source.read_bytes()
            report = calibration.build(source, room, output)
            self.assertEqual(report["cases"][:2], cases)
            self.assertEqual(len(report["cases"]), 10)
            self.assertTrue(all(case["split"] == "calibration" for case in report["cases"]))
            self.assertEqual(source.read_bytes(), frozen)
            first = output.read_bytes()
            self.assertEqual(report, calibration.build(source, room, output))
            self.assertEqual(first, output.read_bytes())
            self.assertEqual(output.stat().st_mode & 0o777, 0o600)
            for case in report["cases"][2:]:
                data = (root / case["audio"]["path"]).read_bytes()
                self.assertEqual(hashlib.sha256(data).hexdigest(), case["audio"]["sha256"])
                self.assertEqual((root / case["audio"]["path"]).stat().st_mode & 0o777, 0o600)
                self.assertTrue(Path(case["audio"]["path"]).name.startswith("calibration-"))
            tails = [case for case in report["cases"] if "trailing-silence" in case["tags"]]
            self.assertEqual([case["reference"] for case in tails], [case["reference"] for case in cases])
            self.assertTrue(all(case["audio"]["frames"] == 48320 for case in tails))
            for tail, original in zip(tails, cases):
                audio = (root / tail["audio"]["path"]).read_bytes()
                pcm = calibration._pcm(audio, tail["audio"]["frames"])
                self.assertEqual(pcm[:640], struct.pack("<320h", *([1000, -1000] * 160)))
                self.assertEqual(pcm[640:], bytes(96000))
            terminal = io.StringIO()
            with redirect_stdout(terminal):
                self.assertEqual(calibration.main(["--source-manifest", str(source), "--room-source", str(room), "--output", str(output)]), 0)
            self.assertIn("no acceptance cases or ASR", terminal.getvalue())

    def test_pin_audio_and_frame_errors_publish_nothing_and_preserve_sources(self):
        for fault in ["manifest", "room", "audio", "frame", "path", "count"]:
            with self.subTest(fault=fault), self.fixture() as (root, source, room, output, cases):
                if fault == "manifest":
                    source.write_bytes(source.read_bytes() + b" ")
                elif fault == "room":
                    room.write_bytes(b"different room")
                elif fault == "audio":
                    (root / cases[0]["audio"]["path"]).write_bytes(b"corrupted")
                elif fault in ("frame", "path", "count"):
                    full = json.loads(source.read_text())
                    if fault == "frame":
                        full["cases"][0]["audio"]["frames"] += 1
                    elif fault == "path":
                        full["cases"][0]["audio"]["path"] = "../outside.wav"
                    else:
                        full["cases"].pop()
                    source.write_text(json.dumps(full))
                    calibration.BASE_SHA256 = hashlib.sha256(source.read_bytes()).hexdigest()
                before = source.read_bytes()
                with self.assertRaises(calibration.CalibrationError):
                    calibration.build(source, room, output)
                self.assertFalse(output.exists())
                self.assertEqual(list((root / "artifacts/corpus/audio").glob("calibration-*")), [])
                self.assertEqual(source.read_bytes(), before)

    def test_frozen_mismatch_and_failed_sync_never_replace_final_output(self):
        with self.fixture() as (root, source, room, output, _):
            output.write_bytes(b"prior frozen calibration")
            with self.assertRaises(calibration.CalibrationError):
                calibration.build(source, room, output)
            self.assertEqual(output.read_bytes(), b"prior frozen calibration")
            self.assertEqual(list((root / "artifacts/corpus/audio").glob("calibration-*")), [])
            output.unlink()
            with mock.patch.object(calibration.os, "fsync", side_effect=OSError("injected disk failure")):
                with self.assertRaises(OSError):
                    calibration.build(source, room, output)
            self.assertFalse(output.exists())
            self.assertEqual(list((root / "artifacts/corpus/audio").glob("calibration-*")), [])
            self.assertEqual(list((root / "artifacts/corpus/audio").glob(".calibration-*")), [])

    def test_noise_seed_is_fixed_new_and_not_an_empty_signal(self):
        first = calibration._noise()
        self.assertEqual(first, calibration._noise())
        self.assertEqual(len(first), 160000)
        self.assertNotEqual(first, bytes(160000))
        self.assertEqual(calibration.NOISE_SEED, 0x45564332)

    def test_unpinned_decoder_and_output_source_alias_fail_before_any_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            fake = Path(directory) / "ffmpeg"
            fake.write_bytes(b"not the pinned decoder")
            with mock.patch.object(calibration, "FFMPEG", fake), mock.patch.object(calibration.subprocess, "run", side_effect=AssertionError("unpinned decoder must not execute")):
                with self.assertRaises(calibration.CalibrationError):
                    calibration.decode_room(b"source")
        with self.fixture() as (root, source, room, _, cases):
            before = source.read_bytes()
            with self.assertRaises(calibration.CalibrationError):
                calibration.build(source, room, source)
            self.assertEqual(source.read_bytes(), before)
            self.assertEqual(list((root / "artifacts/corpus/audio").glob("calibration-*")), [])

    def test_room_decoder_rejects_failure_wrong_length_and_timeout(self):
        # Restore the real external-decoder seam; no actual room/decoder runs.
        decoder = {"version": "frozen fixture", "sha256": "hash"}
        for result in [subprocess.CompletedProcess([], 1, b"", b"private decoder failure"), subprocess.CompletedProcess([], 0, b"short", b"")]:
            with mock.patch.object(calibration, "_decoder_info", return_value=decoder), mock.patch.object(calibration.subprocess, "run", return_value=result):
                with self.assertRaises(calibration.CalibrationError):
                    calibration.decode_room(b"verified source")
        with mock.patch.object(calibration, "_decoder_info", return_value=decoder), mock.patch.object(calibration.subprocess, "run", side_effect=calibration.subprocess.TimeoutExpired([], 30)):
            with self.assertRaises(calibration.CalibrationError):
                calibration.decode_room(b"verified source")

if __name__ == "__main__":
    unittest.main()
