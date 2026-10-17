import errno
import importlib.util
import io
import json
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest import mock

SPEC = importlib.util.spec_from_file_location("short_corpus", Path(__file__).resolve().parents[2] / "scripts" / "short_corpus.py")
short = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(short)


class ShortCorpusTests(unittest.TestCase):
    @staticmethod
    def flac(frames=24000, rate=48000, channels=1, bits=16):
        info = bytes(10) + struct.pack(">Q", (rate << 44) | ((channels - 1) << 41) | ((bits - 1) << 36) | frames) + bytes(16)
        return b"fLaC\x80\x00\x00\x22" + info

    @classmethod
    def archive(cls, extra=None, overlap=False):
        files = {}
        splits = {"train": [], "test": []}
        for language in ["german", "english"]:
            for split, speakers in [("train", [0, 1]), ("test", [4, 5])]:
                for speaker in speakers:
                    for digit in [8, 9]:
                        name = "lang-{}_speaker-{:02d}_trial-0_digit-{}.flac".format(language, speaker, digit)
                        splits[split].append(name)
                        files["audio/" + name] = cls.flac()
        if overlap:
            splits["test"].append(splits["train"][0])
        files.update({key + "_filenames.txt": ("\n".join(value) + "\n").encode() for key, value in splits.items()})
        files["README.md"] = b"fixture metadata"
        if extra:
            files.update(extra)
        out = io.BytesIO()
        with tarfile.open(fileobj=out, mode="w:gz") as archive:
            for name, data in files.items():
                info = tarfile.TarInfo(name)
                info.size = len(data)
                archive.addfile(info, io.BytesIO(data))
        return out.getvalue()

    def test_actual_archive_boundary_selects_eight_independent_word_labels_and_speakers(self):
        with tarfile.open(fileobj=io.BytesIO(self.archive()), mode="r:gz") as archive:
            rows = short.inspect_archive(archive)
        selected = short.select_cases(list(reversed(rows)))
        self.assertEqual(len(selected), 8)
        for language, references in [("de", ["acht", "neun"]), ("en", ["eight", "nine"])]:
            for split, speakers in [("calibration", [0, 1]), ("acceptance", [4, 5])]:
                cases = [row for row in selected if row["language"] == language and row["split"] == split]
                self.assertEqual([row["reference"] for row in cases], references)
                self.assertEqual([row["speaker"] for row in cases], speakers)

    def test_filename_and_duration_guards_do_not_silently_shorten_recordings(self):
        for name in ["../evil.flac", "/audio/evil.flac", "audio/lang-german_speaker-99_trial-0_digit-8.flac", "audio/lang-german_speaker-00_trial-0_digit-8.flac/other"]:
            with self.assertRaises(short.ShortCorpusError):
                short.parse_filename(name)
        info = short.flac_info(self.flac(9600))
        self.assertEqual((info["frames"], info["sampleRate"], info["channels"], info["bitsPerSample"]), (9600, 48000, 1, 16))
        for data in [b"not a FLAC", self.flac(rate=16000), self.flac(channels=2), self.flac(bits=24)]:
            with self.assertRaises(short.ShortCorpusError):
                short.flac_info(data)
        with tarfile.open(fileobj=io.BytesIO(self.archive()), mode="r:gz") as archive:
            rows = short.inspect_archive(archive)
        for row in rows:
            if row["language"] == "de" and row["digit"] == 8:
                row["frames"] = 48001
        with self.assertRaises(short.ShortCorpusError):
            short.select_cases(rows)
        for row in rows:
            if row["language"] == "de" and row["digit"] == 8:
                row["frames"] = 9599
        with self.assertRaises(short.ShortCorpusError):
            short.select_cases(rows)

    def test_unsafe_tar_member_and_overlapping_official_splits_are_rejected(self):
        for data in [self.archive({"../escape": b"no"}), self.archive(overlap=True)]:
            with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive, self.assertRaises(short.ShortCorpusError):
                short.inspect_archive(archive)
        out = io.BytesIO()
        with tarfile.open(fileobj=out, mode="w:gz") as archive:
            link = tarfile.TarInfo("audio/lang-german_speaker-00_trial-0_digit-8.flac")
            link.type, link.linkname = tarfile.SYMTYPE, "../../outside"
            archive.addfile(link)
        with tarfile.open(fileobj=io.BytesIO(out.getvalue()), mode="r:gz") as archive, self.assertRaises(short.ShortCorpusError):
            short.inspect_archive(archive)

    def test_hash_guard_and_frozen_writes_reject_changes_without_overwriting(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "wrong.tar.gz"
            path.write_bytes(b"untrusted bytes")
            with self.assertRaises(short.ShortCorpusError):
                with short.verified_source(path):
                    self.fail("unverified archive became readable")
            with mock.patch.object(short, "SOURCE_BYTES", path.stat().st_size), self.assertRaises(short.ShortCorpusError):
                with short.verified_source(path):
                    self.fail("right size alone bypassed SHA256 verification")
            frozen = Path(directory) / "frozen.json"
            short.write_identical(frozen, b"unchanged")
            self.assertEqual(frozen.stat().st_mode & 0o777, 0o600)
            short.write_identical(frozen, b"unchanged")
            with self.assertRaises(short.ShortCorpusError):
                short.write_identical(frozen, b"changed")
            self.assertEqual(frozen.read_bytes(), b"unchanged")
            link = Path(directory) / "alias.json"
            link.symlink_to(frozen)
            with self.assertRaises(short.ShortCorpusError):
                short.write_identical(link, b"unchanged")

    def test_manifest_extension_preserves_base_and_only_completes_short_coverage(self):
        base = {"schemaVersion": 1, "unicodeVersion": "13.0.0", "sources": [], "selectionRules": {}, "requiredCoveragePending": ["natural_de_utterances_0.2_to_1s", "natural_en_utterances_0.2_to_1s", "natural_room_noise", "independent_listening_and_anchor_annotation_numbers_names_negations"], "cases": [{"id": "base"}]}
        original = json.dumps(base, sort_keys=True)
        cases = [{"id": "hd-{}-{}-{}".format(language, split, digit), "language": language, "split": split, "reference": {"de": {8: "acht", 9: "neun"}, "en": {8: "eight", 9: "nine"}}[language][digit], "audio": {"frames": 8000, "sampleRate": 16000}} for language in ["de", "en"] for split in ["calibration", "acceptance"] for digit in [8, 9]]
        result = short.append_manifest(base, cases, {"version": "fixture", "sha256": "0" * 64})
        self.assertEqual(json.dumps(base, sort_keys=True), original)
        self.assertEqual(len(result["cases"]), 9)
        self.assertEqual(result["requiredCoveragePending"], ["natural_room_noise", "independent_listening_and_anchor_annotation_numbers_names_negations", "exact_30s_duration_cases"])
        self.assertEqual(result["sources"][-1]["license"], "CC-BY-4.0")
        incomplete = short.append_manifest(base, cases[:-1], {"version": "fixture", "sha256": "0" * 64})
        self.assertIn("natural_en_utterances_0.2_to_1s", incomplete["requiredCoveragePending"])
        with self.assertRaises(short.ShortCorpusError):
            short.append_manifest(result, cases, {"version": "fixture", "sha256": "0" * 64})

    def test_manifest_byte_limit_and_output_alias_protect_frozen_input(self):
        with tempfile.TemporaryDirectory() as directory:
            base = Path(directory) / "base.json"
            original = b'{"schemaVersion":1,"unicodeVersion":"13.0.0","cases":[]}'
            base.write_bytes(original)
            with self.assertRaises(short.ShortCorpusError):
                short.build(base, Path(directory) / "unneeded.tar.gz", base)
            self.assertEqual(base.read_bytes(), original)
            with base.open("wb") as output:
                output.seek(short.MAX_JSON_BYTES)
                output.write(b" ")
            with self.assertRaises(short.ShortCorpusError):
                short.read_base(base)

    def test_failed_flush_never_publishes_partial_final_file(self):
        with tempfile.TemporaryDirectory() as directory:
            final = Path(directory) / "frozen.json"
            with mock.patch.object(short.os, "fsync", side_effect=OSError("controlled storage failure")), self.assertRaises(OSError):
                short.write_identical(final, b"complete bytes")
            self.assertFalse(final.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])

    def test_partial_enospc_write_leaves_no_final_or_temporary_file(self):
        original = tempfile.NamedTemporaryFile
        def failing_tempfile(*args, **kwargs):
            target = original(*args, **kwargs)
            write = target.write
            def fail_after_prefix(data):
                write(data[:3])
                raise OSError(errno.ENOSPC, "controlled write failure")
            target.write = fail_after_prefix
            return target
        with tempfile.TemporaryDirectory() as directory:
            final = Path(directory) / "frozen.wav"
            with mock.patch.object(short.tempfile, "NamedTemporaryFile", side_effect=failing_tempfile), self.assertRaises(OSError):
                short.write_identical(final, b"complete waveform bytes")
            self.assertFalse(final.exists())
            self.assertEqual(list(Path(directory).iterdir()), [])
            short.write_identical(final, b"complete waveform bytes")
            self.assertEqual(final.read_bytes(), b"complete waveform bytes")

    def test_in_place_source_change_during_decode_prevents_all_publication(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source, base, final = root / "hd.tar.gz", root / "base.json", root / "final.json"
            original = self.archive()
            source.write_bytes(original)
            base.write_text(json.dumps({"schemaVersion": 1, "unicodeVersion": "13.0.0", "cases": []}))
            def changed_source(_data, _info):
                with source.open("r+b") as output:
                    output.seek(len(original) - 1)
                    output.write(bytes([original[-1] ^ 1]))
                return bytes(44 + 8000 * 2)
            with mock.patch.object(short, "SOURCE_BYTES", len(original)), mock.patch.object(short, "SOURCE_SHA256", short.sha256(original)), mock.patch.object(short, "tool_identity", return_value={"version": "fixture", "sha256": "0" * 64}), mock.patch.object(short, "normalize_flac", side_effect=changed_source), self.assertRaises(short.ShortCorpusError):
                short.build(base, source, final, root=root)
            self.assertFalse(final.exists())
            self.assertFalse((root / "artifacts").exists())


if __name__ == "__main__":
    unittest.main()
